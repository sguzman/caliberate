use caliberate_core::error::CoreResult;
use std::path::{Component, Path, PathBuf};
pub(super) fn safe_path(root: &Path, book: &str, name: &str, format: &str) -> CoreResult<PathBuf> {
    let p = Path::new(book);
    if p.is_absolute()
        || p.components().any(|x| {
            matches!(
                x,
                Component::Prefix(_) | Component::RootDir | Component::ParentDir
            )
        })
    {
        return Err(super::incompatible("unsafe Calibre books.path"));
    }
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || Path::new(name).components().count() != 1
    {
        return Err(super::incompatible("unsafe Calibre data.name"));
    }
    let format_path = Path::new(format);
    if format.is_empty()
        || format.contains('/')
        || format.contains('\\')
        || format_path.components().count() != 1
        || format_path.components().any(|x| {
            matches!(
                x,
                Component::Prefix(_) | Component::RootDir | Component::ParentDir
            )
        })
        || !matches!(format_path.components().next(), Some(Component::Normal(_)))
    {
        return Err(super::incompatible("unsafe Calibre data.format"));
    }
    let out = root
        .join(p)
        .join(format!("{name}.{}", format.to_ascii_lowercase()));
    if !out.starts_with(root) {
        return Err(super::incompatible(
            "Calibre content path escapes library root",
        ));
    }
    Ok(out)
}
