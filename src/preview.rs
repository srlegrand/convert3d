//! Previews: an interactive viewer page opened in the web browser, and
//! thumbnails rendered by Blender for the file manager.

use crate::convert;
use crate::formats::{self, Tool};
use crate::plan::{self, Actor, Overrides};
use crate::tools::Tools;
use crate::ui::{self, Log};
use anyhow::{Context, Result, bail};
use base64::Engine;
use std::fs;
use std::path::{Path, PathBuf};

const VIEWER_HTML: &str = include_str!("../viewer/viewer.html");
const VIEWER_JS: &str = include_str!("../viewer/dist/viewer.min.js");
const KEEP_PAGES: usize = 20;

/// True when viewing `input` means converting it first, which takes a few seconds.
pub fn needs_conversion(input: &Path) -> bool {
    formats::ext_of(input) != "glb"
}

/// Writes a self-contained viewer page for `input` and returns its path.
pub fn view(input: &Path, tools: &Tools, overrides: &Overrides, log: &mut Log) -> Result<PathBuf> {
    let input = input.canonicalize().with_context(|| format!("{} not found", input.display()))?;
    let name = input.file_name().unwrap_or_default().to_string_lossy().to_string();
    let models = if needs_conversion(&input) {
        let work = tempfile::Builder::new().prefix("convert3d-").tempdir()?;
        let staging = convert::stage(&input, "glb", work.path(), tools, overrides, log)?;
        let mut glbs: Vec<_> = fs::read_dir(&staging)?
            .flatten()
            .map(|e| e.path())
            .filter(|p| formats::ext_of(p) == "glb")
            .collect();
        glbs.sort();
        glbs.iter()
            .map(|p| Ok((p.file_name().unwrap().to_string_lossy().to_string(), fs::read(p)?)))
            .collect::<Result<Vec<_>>>()?
    } else {
        vec![(name.clone(), fs::read(&input)?)]
    };
    if models.is_empty() {
        bail!("conversion produced no model to show");
    }

    let models_json = serde_json::Value::Array(
        models
            .iter()
            .map(|(n, bytes)| {
                serde_json::json!({
                    "name": n,
                    "glb": base64::engine::general_purpose::STANDARD.encode(bytes),
                })
            })
            .collect(),
    );
    let page = fill(
        VIEWER_HTML,
        &[
            ("/*TITLE*/", &script_safe(&serde_json::Value::from(name.as_str()).to_string())),
            ("/*MODELS*/", &script_safe(&models_json.to_string())),
            ("/*VIEWER*/", VIEWER_JS),
        ],
    )?;

    let dir = ui::cache_dir().join("previews");
    fs::create_dir_all(&dir)?;
    ui::prune(&dir, KEEP_PAGES - 1);
    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
    let safe: String = stem.chars().map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' }).collect();
    let path = dir.join(format!("{}-{safe}.html", ui::timestamp()));
    fs::write(&path, page)?;
    Ok(path)
}

/// Renders a `size`×`size` PNG of `input` to `output`, converting first when
/// Blender can't read it directly.
pub fn thumbnail(input: &Path, output: &Path, size: u32, tools: &Tools, log: &mut Log) -> Result<()> {
    let input = input.canonicalize().with_context(|| format!("{} not found", input.display()))?;
    let from = formats::ext_of(&input);
    let format = formats::get(&from).with_context(|| format!("unsupported format .{from}"))?;
    let work = tempfile::Builder::new().prefix("convert3d-").tempdir()?;
    let search_dirs = vec![input.parent().unwrap().to_path_buf()];

    let mut current = input.clone();
    if !format.read.contains(&Tool::Blender) {
        // With the shared tools every route to .glb ends with Blender, so
        // stop before that step and render what it would have read. Options
        // are left out: a thumbnailer must not depend on per-format choices.
        let overrides = Overrides::default();
        let steps = plan::plan_with(&from, "glb", &tools.available(), &overrides)?;
        if steps.last().is_some_and(|s| s.actor != Actor::Tool(Tool::Blender)) {
            bail!("no route from .{from} to something Blender can render");
        }
        for (i, step) in steps.iter().take(steps.len().saturating_sub(1)).enumerate() {
            let out = work.path().join(format!("step{i}.{}", step.to));
            let scratch = work.path().join(format!("step{i}"));
            fs::create_dir_all(&scratch)?;
            tools.run(step, &overrides, &current, &out, &search_dirs, &scratch, log)?;
            current = out;
        }
    }

    // Blender adds ".png" to a path without it, and file managers often pass one.
    let png = work.path().join("thumbnail.png");
    let scratch = work.path().join("render");
    fs::create_dir_all(&scratch)?;
    tools.render_thumbnail(&current, &png, size, &search_dirs, &scratch, log)?;
    fs::copy(&png, output).with_context(|| format!("could not write {}", output.display()))?;
    Ok(())
}

/// Replaces each marker, in order, with its value. Values aren't searched
/// for later markers.
fn fill(template: &str, parts: &[(&str, &str)]) -> Result<String> {
    let mut out = String::new();
    let mut rest = template;
    for (marker, value) in parts {
        let (before, after) = rest.split_once(marker).with_context(|| format!("template lacks {marker}"))?;
        out.push_str(before);
        out.push_str(value);
        rest = after;
    }
    out.push_str(rest);
    Ok(out)
}

/// JSON that can sit inside a <script> element. `<` only occurs inside
/// strings, where `\u003c` means the same.
fn script_safe(json: &str) -> String {
    json.replace('<', "\\u003c")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_in_order() {
        let t = "a /*X*/ b /*Y*/ c";
        assert_eq!(fill(t, &[("/*X*/", "/*Y*/"), ("/*Y*/", "2")]).unwrap(), "a /*Y*/ b 2 c");
        assert!(fill(t, &[("/*Z*/", "")]).is_err());
    }

    #[test]
    fn viewer_template_has_markers() {
        assert!(fill(VIEWER_HTML, &[("/*TITLE*/", ""), ("/*MODELS*/", ""), ("/*VIEWER*/", "")]).is_ok());
        assert!(!VIEWER_JS.contains("</script"));
    }

    #[test]
    fn script_safe_escapes_closing_tags() {
        assert_eq!(script_safe(r#""</script>""#), r#""\u003c/script>""#);
    }
}
