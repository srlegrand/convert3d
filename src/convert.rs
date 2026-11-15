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

/// Names a conversion may write beside the source: one file, a folder of
/// several files, or a zip when the input itself is a zip of several models.
pub fn candidate_names(input: &Path, to: &str) -> Vec<String> {
    let raw = input.file_stem().unwrap_or_default().to_string_lossy();
    let stem = logical_stem(&raw);
    let to = to.trim_start_matches('.').to_ascii_lowercase();
    let mut names = vec![format!("{stem}.{to}"), format!("{stem}_{to}")];
    if formats::is_zip(input) {
        names.push(format!("{stem}_{to}.zip"));
    }
    names
}

/// `chair (2)` and `chair_(2)` are both `chair`. Repeated suffixes come off too,
/// so `chair (2) (2)` is still `chair`.
fn logical_stem(stem: &str) -> &str {
    let mut stem = stem;
    while let Some(stripped) = strip_increment(stem) {
        if stripped.is_empty() {
            break;
        }
        stem = stripped;
    }
    stem
}

/// One trailing ` (12)` or `_(12)`. The space or underscore belongs to the suffix.
fn strip_increment(stem: &str) -> Option<&str> {
    let (root, _) = one_increment(stem)?;
    Some(root)
}

fn one_increment(stem: &str) -> Option<(&str, u64)> {
    let open = stem.rfind('(')?;
    if open == 0 || !stem.ends_with(')') {
        return None;
    }
    let sep = stem.as_bytes()[open - 1];
    if sep != b' ' && sep != b'_' {
        return None;
    }
    let digits = &stem[open + 1..stem.len() - 1];
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let number = digits.parse().ok()?;
    Some((&stem[..open - 1], number))
}

pub fn convert(
    input: &Path,
    opts: &Options,
    tools: &Tools,
    overrides: &plan::Overrides,
    log: &mut Log,
) -> Result<PathBuf> {
    let input = input.canonicalize().with_context(|| format!("{} not found", input.display()))?;
    let to = opts.to.trim_start_matches('.').to_ascii_lowercase();
    if formats::get(&to).is_none_or(|f| f.write.is_empty()) {
        bail!("cannot write .{to}");
    }
    let work = tempfile::Builder::new().prefix("convert3d-").tempdir()?;
    let staging = work.path().join("out");
    fs::create_dir_all(&staging)?;

    let from_zip = formats::is_zip(&input);
    let raw_stem = input.file_stem().unwrap_or_default().to_string_lossy();
    let stem = logical_stem(&raw_stem).to_string();
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
        let mut steps = plan::plan_with(&from, &to, &tools.available(), overrides)?;
        if steps.is_empty() {
            // Same format out of a zip: re-export so textures get embedded/collected.
            steps = plan::reexport_with(&from, &tools.available(), overrides)?;
        }
        log.line(&format!(
            "{} -> .{to} via {}",
            model.display(),
            steps.iter().map(|s| format!("{}→.{}", s.actor.label(), s.to)).collect::<Vec<_>>().join(", ")
        ));
        let name = if models.len() == 1 {
            stem.clone()
        } else {
            let raw = model.file_stem().unwrap_or_default().to_string_lossy();
            logical_stem(&raw).to_string()
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
            tools.run(step, overrides, &current, &out, &search_dirs, &scratch, log)?;
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

/// `parent/name`, or `parent/name_(2)` when that name is taken.
/// `name (2)` and `name_(2)` are the same file as `name`, so a second copy is
/// `name_(3)` rather than `name (2) (2)`.
fn target_path(parent: &Path, name: &str, overwrite: bool) -> PathBuf {
    let (base, ext) = split_name(name);
    let first = parent.join(format!("{base}{ext}"));
    if overwrite || !first.exists() {
        return first;
    }
    let taken = taken_numbers(parent, &base, &ext);
    let n = (2u64..).find(|n| !taken.contains(n)).unwrap();
    parent.join(format!("{base}_({n}){ext}"))
}

fn split_name(name: &str) -> (String, String) {
    match name.rsplit_once('.') {
        Some((base, ext)) if !base.is_empty() => (logical_stem(base).to_string(), format!(".{ext}")),
        _ => (logical_stem(name).to_string(), String::new()),
    }
}

fn taken_numbers(parent: &Path, base: &str, ext: &str) -> Vec<u64> {
    let mut nums = Vec::new();
    let Ok(dir) = fs::read_dir(parent) else { return nums };
    for entry in dir.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let (stem, file_ext) = match name.rsplit_once('.') {
            Some((stem, ext)) if !stem.is_empty() => (stem, format!(".{ext}")),
            _ => (name.as_ref(), String::new()),
        };
        if !file_ext.eq_ignore_ascii_case(ext) {
            continue;
        }
        let Some((root, n)) = one_increment(stem) else { continue };
        if root.eq_ignore_ascii_case(base) {
            nums.push(n);
        }
    }
    nums
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
    fn candidate_names_cover_file_folder_and_zip() {
        assert_eq!(candidate_names(Path::new("C:/models/cube.ma"), ".usda"), ["cube.usda", "cube_usda"]);
        assert_eq!(
            candidate_names(Path::new("C:/models/cube (2) (2).ma"), "usda"),
            ["cube.usda", "cube_usda"]
        );
        assert_eq!(
            candidate_names(Path::new("C:/models/cube.zip"), "glb"),
            ["cube.glb", "cube_glb", "cube_glb.zip"]
        );
    }

    #[test]
    fn unique_names() {
        let d = tempfile::tempdir().unwrap();
        touch(&d.path().join("a.glb"));
        assert_eq!(target_path(d.path(), "a.glb", false), d.path().join("a_(2).glb"));
        assert_eq!(target_path(d.path(), "a.glb", true), d.path().join("a.glb"));
    }

    #[test]
    fn increment_strips_stacked_suffixes_and_counts_both_spellings() {
        let d = tempfile::tempdir().unwrap();
        touch(&d.path().join("Natural_History.ma"));
        touch(&d.path().join("Natural_History (2).ma"));
        touch(&d.path().join("Natural_History (2) (2).ma"));
        touch(&d.path().join("Natural_History_(4).ma"));
        assert_eq!(
            target_path(d.path(), "Natural_History (2) (2) (2).ma", false),
            d.path().join("Natural_History_(3).ma")
        );
        assert_eq!(
            target_path(d.path(), "Natural_History_(2).usda", false),
            d.path().join("Natural_History.usda")
        );
    }
}
