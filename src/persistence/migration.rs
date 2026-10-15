//! One-time, non-destructive import of the previous installation's local library.
use super::*;

pub fn migrate_previous_installation() -> Result<(), String> {
    if std::env::var_os("ALPHADOCS_DATA_DIR").is_some()
        || std::env::var_os(crate::legacy::DATA_ENV).is_some()
    {
        return Ok(());
    }
    let Some(previous) = ProjectDirs::from(
        "studio",
        crate::legacy::ORGANISATION,
        crate::legacy::ORGANISATION,
    ) else {
        return Ok(());
    };
    migrate_from(previous.data_local_dir(), &data_dir())
}

fn migrate_from(source: &Path, destination: &Path) -> Result<(), String> {
    let marker = destination.join("previous-library-imported.json");
    if !source.is_dir() || marker.exists() || source == destination {
        return Ok(());
    }
    for name in ["preferences.json", "session.json"] {
        let path = source.join(name);
        if path.exists() {
            copy_new(&path, &destination.join(name))?;
        }
    }
    for name in ["documents", "history", "trash"] {
        let path = source.join(name);
        if path.exists() {
            copy_tree(&path, &destination.join(name))?;
        }
    }
    atomic_write(&marker, b"{\"version\":1,\"originals_retained\":true}")
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(source).map_err(|e| e.to_string())?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(
            "Previous library contains a linked or unsupported directory; originals were retained"
                .into(),
        );
    }
    fs::create_dir_all(destination).map_err(|e| e.to_string())?;
    if fs::symlink_metadata(destination)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("Library destination must be a normal directory".into());
    }
    for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        let mut target = destination.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if kind.is_file() {
            if target
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case(crate::legacy::EXTENSION))
            {
                target.set_extension("adocs");
            }
            copy_new(&entry.path(), &target)?;
        } else {
            return Err(
                "Previous library contains a link or unsupported entry; originals were retained"
                    .into(),
            );
        }
    }
    Ok(())
}

fn copy_new(source: &Path, destination: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(source).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("Previous library entry is not a normal file".into());
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut input = File::open(source).map_err(|e| e.to_string())?;
    let mut output = match File::options()
        .write(true)
        .create_new(true)
        .open(destination)
    {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Ok(()),
        Err(e) => return Err(e.to_string()),
    };
    let result = std::io::copy(&mut input, &mut output).and_then(|_| output.sync_all());
    drop(output);
    if let Err(e) = result {
        let _ = fs::remove_file(destination);
        return Err(e.to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migration_preserves_originals_existing_files_history_and_corrupt_bytes() {
        let root =
            std::env::temp_dir().join(format!("alphadocs-migration-{}", Document::new("").id));
        let old = root.join("previous");
        let new = root.join("current");
        let doc = Document::new("Café 😀");
        let filename = format!("{}.{}", doc.id, crate::legacy::EXTENSION);
        save_document(&doc, &old.join("documents").join(&filename)).unwrap();
        atomic_write(
            &old.join("history").join(&doc.id).join(&filename),
            b"original history",
        )
        .unwrap();
        atomic_write(
            &old.join("trash").join(&filename),
            b"corrupt original retained",
        )
        .unwrap();
        atomic_write(&old.join("preferences.json"), b"{\"autosave\":false}").unwrap();
        atomic_write(&old.join("session.json"), b"{\"documents\":[]}").unwrap();
        atomic_write(&new.join("preferences.json"), b"{\"theme\":\"Dark\"}").unwrap();
        let original = fs::read(old.join("documents").join(&filename)).unwrap();
        migrate_from(&old, &new).unwrap();
        assert_eq!(
            load_document(&new.join("documents").join(format!("{}.adocs", doc.id))).unwrap(),
            doc
        );
        assert_eq!(
            fs::read(new.join("preferences.json")).unwrap(),
            b"{\"theme\":\"Dark\"}"
        );
        assert_eq!(
            fs::read(new.join("trash").join(format!("{}.adocs", doc.id))).unwrap(),
            b"corrupt original retained"
        );
        assert_eq!(
            fs::read(
                new.join("history")
                    .join(&doc.id)
                    .join(format!("{}.adocs", doc.id))
            )
            .unwrap(),
            b"original history"
        );
        assert_eq!(
            fs::read(old.join("documents").join(&filename)).unwrap(),
            original
        );
        atomic_write(
            &new.join("documents").join(format!("{}.adocs", doc.id)),
            b"newer file",
        )
        .unwrap();
        migrate_from(&old, &new).unwrap();
        assert_eq!(
            fs::read(new.join("documents").join(format!("{}.adocs", doc.id))).unwrap(),
            b"newer file"
        );
        // Only this test's generated temporary directory is removed.
        let resolved = root.canonicalize().unwrap();
        assert!(resolved.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        assert!(
            resolved
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("alphadocs-migration-")
        );
        fs::remove_dir_all(resolved).unwrap();
    }
}
