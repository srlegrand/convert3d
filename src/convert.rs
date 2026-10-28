//! One conversion job: unpack a zip if needed, run the planned tool chain,
//! and put the result next to the source.

use crate::formats::{self, Tool};
use crate::plan;
use crate::tools::Tools;
use crate::ui::Log;
use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct Options {
    pub to: String,
    pub out_dir: Option<PathBuf>,
    pub overwrite: bool,
}

const STEP_PREFIX: &str = "__c3d_step";

pub fn convert(input: &Path, opts: &Options, tools: &Tools, log: &mut Log) -> Result<PathBuf> {
    let input = input.canonicalize().with_context(|| format!("{} not found", input.display()))?;
    let to = opts.to.trim_start_matches('.').to_ascii_lowercase();
    if formats::get(&to).is_none_or(|f| f.write.is_empty()) {
        bail!("cannot write .{to}");
    }
    let work = tempfile::Builder::new().prefix("convert3d-").tempdir()?;
    let staging = work.path().join("out");
    fs::create_dir_all(&staging)?;

    let from_zip = formats::is_zip(&input);
    let stem = input.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let (models, search_dirs) = if from_zip {
        let dir = work.path().join("src");
        extract_zip(&input, &dir)?;
        let models = pick_models(&dir, &tools.available())?;
        log.line(&format!("models in zip: {models:?}"));
        (models, vec![dir])
    } else {
        if formats::ext_of(&input) == to {
            bail!("{} is already .{to}", input.display());
        }
        (vec![input.clone()], vec![input.parent().unwrap().to_path_buf()])
    };

    for (n, model) in models.iter().enumerate() {
        let from = formats::ext_of(model);
        let mut steps = plan::plan(&from, &to, &tools.available())?;
        if steps.is_empty() {
            // Same format out of a zip: re-export so textures get embedded/collected.
            steps = plan::reexport(&from, &tools.available())?;
        }
        log.line(&format!(
            "{} -> .{to} via {}",
            model.display(),
            steps.iter().map(|s| format!("{}→.{}", s.tool.name(), s.to)).collect::<Vec<_>>().join(", ")
        ));
        let name = if models.len() == 1 {
            stem.clone()
        } else {
            model.file_stem().unwrap_or_default().to_string_lossy().to_string()
        };
        let mut current = model.clone();
        for (i, step) in steps.iter().enumerate() {
            let last = i + 1 == steps.len();
            let file =
                if last { format!("{name}.{to}") } else { format!("{STEP_PREFIX}{n}_{i}.{}", step.to) };
            let out = staging.join(file);
            // Each step gets its own scratch dir for scripts and unpacked textures.
            let scratch = work.path().join(format!("step{n}_{i}"));
            fs::create_dir_all(&scratch)?;
            tools.run(step.tool, &current, &out, &search_dirs, &scratch, log)?;
            current = out;
        }
    }

    // Drop intermediate files (and glTF .bin buffers that belong to them).
    for entry in fs::read_dir(&staging)? {
        let p = entry?.path();
        if p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(STEP_PREFIX)) {
            remove_any(&p)?;
        }
    }

    let out_parent = match &opts.out_dir {
        Some(d) => d.clone(),
        None => input.parent().unwrap().to_path_buf(),
    };
    fs::create_dir_all(&out_parent)?;
    place_output(&staging, &out_parent, &stem, &to, from_zip, opts.overwrite)
}

fn extract_zip(zip: &Path, dest: &Path) -> Result<()> {
    let file = fs::File::open(zip)?;
    let mut archive =
        zip::ZipArchive::new(file).with_context(|| format!("{} is not a valid zip", zip.display()))?;
    // `extract` refuses entries that would escape `dest`.
    archive.extract(dest)?;
    Ok(())
}

/// Chooses which file(s) in an extracted zip to convert: the highest
/// priority model format, at the shallowest folder depth.
pub fn pick_models(dir: &Path, available: &[Tool]) -> Result<Vec<PathBuf>> {
    let mut found: Vec<(PathBuf, usize)> = WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| {
            let n = e.file_name().to_string_lossy();
            e.depth() == 0 || !(n.starts_with('.') || n == "__MACOSX")
        })
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter(|e| formats::get(&formats::ext_of(e.path())).is_some_and(|f| !f.read.is_empty()))
        .map(|e| (e.path().to_path_buf(), e.depth()))
        .collect();
    if found.is_empty() {
        bail!("no 3D model found in the zip");
    }
    let readable = |p: &Path| {
        formats::get(&formats::ext_of(p)).is_some_and(|f| f.read.iter().any(|t| available.contains(t)))
    };
    // Prefer something we can read; otherwise keep going so the error names the missing tool.
    if found.iter().any(|(p, _)| readable(p)) {
        found.retain(|(p, _)| readable(p));
    }
    let rank = |p: &Path| {
        let ext = formats::ext_of(p);
        formats::ZIP_PRIORITY.iter().position(|e| *e == ext).unwrap_or(formats::ZIP_PRIORITY.len())
    };
    let best_rank = found.iter().map(|(p, _)| rank(p)).min().unwrap();
    let best_ext = found.iter().find(|(p, _)| rank(p) == best_rank).map(|(p, _)| formats::ext_of(p)).unwrap();
    found.retain(|(p, _)| formats::ext_of(p) == best_ext);
    let min_depth = found.iter().map(|(_, d)| *d).min().unwrap();
    found.retain(|(_, d)| *d == min_depth);
    found.sort();
    let mut models: Vec<_> = found.into_iter().map(|(p, _)| p).collect();
    // USD files next to each other are usually layers of one stage.
    if best_ext.starts_with("usd") {
        models.truncate(1);
    }
    Ok(models)
}

/// Moves the converted files out of `staging`: a lone file goes next to the
/// source; several files become a folder (or a zip, when the input was a zip).
fn place_output(
    staging: &Path,
    parent: &Path,
    stem: &str,
    to: &str,
    zip_out: bool,
    overwrite: bool,
) -> Result<PathBuf> {
    let entries: Vec<PathBuf> = fs::read_dir(staging)?.flatten().map(|e| e.path()).collect();
    if entries.len() == 1 && entries[0].is_file() {
        let name = entries[0].file_name().unwrap().to_string_lossy().to_string();
        let dest = target_path(parent, &name, overwrite);
        move_any(&entries[0], &dest)?;
        return Ok(dest);
    }
    if zip_out {
        let dest = target_path(parent, &format!("{stem}_{to}.zip"), overwrite);
        write_zip(staging, &dest)?;
        return Ok(dest);
    }
    let dest = target_path(parent, &format!("{stem}_{to}"), overwrite);
    if dest.exists() {
        remove_any(&dest)?;
    }
    move_any(staging, &dest)?;
    Ok(dest)
}

/// `parent/name`, or `parent/name (2)` etc. when that already exists.
fn target_path(parent: &Path, name: &str, overwrite: bool) -> PathBuf {
    let first = parent.join(name);
    if overwrite || !first.exists() {
        return first;
    }
    let (base, ext) = match name.rsplit_once('.') {
        Some((b, e)) if !b.is_empty() => (b.to_string(), format!(".{e}")),
        _ => (name.to_string(), String::new()),
    };
    (2..).map(|i| parent.join(format!("{base} ({i}){ext}"))).find(|p| !p.exists()).unwrap()
}

fn write_zip(dir: &Path, dest: &Path) -> Result<()> {
    let mut zw = zip::ZipWriter::new(fs::File::create(dest)?);
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for entry in WalkDir::new(dir).min_depth(1) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(dir)?.to_string_lossy().replace('\\', "/");
        if entry.file_type().is_dir() {
            zw.add_directory(rel, opts)?;
        } else {
            zw.start_file(rel, opts)?;
            std::io::copy(&mut fs::File::open(entry.path())?, &mut zw)?;
        }
    }
    zw.finish()?;
    Ok(())
}

fn move_any(from: &Path, to: &Path) -> Result<()> {
    if to.exists() {
        remove_any(to)?;
    }
    if fs::rename(from, to).is_ok() {
        return Ok(());
    }
    // Rename fails across filesystems (e.g. /tmp on tmpfs): copy instead.
    if from.is_dir() {
        for entry in WalkDir::new(from) {
            let entry = entry?;
            let target = to.join(entry.path().strip_prefix(from)?);
            if entry.file_type().is_dir() {
                fs::create_dir_all(&target)?;
            } else {
                fs::copy(entry.path(), &target)?;
            }
        }
    } else {
        fs::copy(from, to)?;
    }
    Ok(())
}

fn remove_any(p: &Path) -> Result<()> {
    if p.is_dir() {
        fs::remove_dir_all(p)?
    } else {
        fs::remove_file(p)?
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(p: &Path) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, b"x").unwrap();
    }

    #[test]
    fn zip_prefers_fbx_at_top_level() {
        let d = tempfile::tempdir().unwrap();
        touch(&d.path().join("chair.obj"));
        touch(&d.path().join("chair.fbx"));
        touch(&d.path().join("lod/chair_lod1.fbx"));
        touch(&d.path().join("__MACOSX/._chair.fbx"));
        touch(&d.path().join("textures/wood.png"));
        let m = pick_models(d.path(), &[Tool::Blender]).unwrap();
        assert_eq!(m, [d.path().join("chair.fbx")]);
    }

    #[test]
    fn zip_skips_unreadable_when_possible() {
        let d = tempfile::tempdir().unwrap();
        touch(&d.path().join("scene.max"));
        touch(&d.path().join("scene.obj"));
        let m = pick_models(d.path(), &[Tool::Blender]).unwrap();
        assert_eq!(m, [d.path().join("scene.obj")]);
    }

    #[test]
    fn unique_names() {
        let d = tempfile::tempdir().unwrap();
        touch(&d.path().join("a.glb"));
        assert_eq!(target_path(d.path(), "a.glb", false), d.path().join("a (2).glb"));
        assert_eq!(target_path(d.path(), "a.glb", true), d.path().join("a.glb"));
    }
}
