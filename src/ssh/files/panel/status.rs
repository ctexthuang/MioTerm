use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    path::Path,
};

use ely_gpui_component::lists::GitStatus;

use crate::{git::Backend, ssh::files::RemotePath};

pub(super) fn key(local: bool, name: &[u8]) -> Cow<'_, [u8]> {
    #[cfg(target_os = "macos")]
    if local
        && let Ok(text) = std::str::from_utf8(name)
        && !unicode_normalization::is_nfc(text)
    {
        use unicode_normalization::UnicodeNormalization;
        return Cow::Owned(text.nfc().collect::<String>().into_bytes());
    }
    let _ = local;
    Cow::Borrowed(name)
}

#[derive(Default, PartialEq)]
pub(super) struct Marks {
    pub direct: HashMap<Vec<u8>, GitStatus>,
    pub dirty: HashSet<Vec<u8>>,
}

pub(super) fn read(backend: &Backend, root: &RemotePath) -> Result<Marks, String> {
    let folder = root.local_path().map_err(|error| format!("{error:#}"))?;
    let changes = backend
        .changes(&folder)
        .map_err(|error| format!("{error:#}"))?;
    let mut marks = Marks::default();
    for (path, status) in changes.unwrap_or_default() {
        let relative = relative(backend, &folder, &path);
        if let Some((name, nested)) = relative {
            if nested {
                marks.dirty.insert(name);
            } else {
                marks.direct.insert(name, status);
            }
        }
    }
    Ok(marks)
}

fn relative(backend: &Backend, root: &Path, path: &Path) -> Option<(Vec<u8>, bool)> {
    if matches!(backend, Backend::Remote(_)) {
        let root = root.as_os_str().as_encoded_bytes();
        let rest = path.as_os_str().as_encoded_bytes().strip_prefix(root)?;
        let rest = if root.ends_with(b"/") {
            rest
        } else {
            rest.strip_prefix(b"/")?
        };
        let mut parts = rest.splitn(2, |byte| *byte == b'/');
        let name = parts.next()?.to_vec();
        return (!name.is_empty())
            .then(|| (name, parts.next().is_some_and(|rest| !rest.is_empty())));
    }
    let (root, path) = (crate::files::mark_key(root), crate::files::mark_key(path));
    let mut parts = path.strip_prefix(root).ok()?.components();
    let name = parts.next()?.as_os_str().as_encoded_bytes().to_vec();
    Some((name, parts.next().is_some()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn only_local_marks_normalize_decomposed_names() {
        assert_eq!(
            key(true, "cafe\u{301}.txt".as_bytes()),
            "café.txt".as_bytes()
        );
        assert_eq!(
            key(false, "cafe\u{301}.txt".as_bytes()),
            "cafe\u{301}.txt".as_bytes()
        );
        assert_eq!(
            relative(
                &Backend::Local,
                Path::new("/cafe\u{301}"),
                Path::new("/café/cafe\u{301}.txt")
            ),
            Some(("café.txt".as_bytes().to_vec(), false))
        );
    }

    #[test]
    fn remote_marks_use_posix_components_and_keep_backslashes() {
        let backend = Backend::Remote(crate::ssh::remote::Remote {
            session: 0,
            host: Default::default(),
            socket: None,
        });
        let root = Path::new("/repo");
        assert_eq!(
            relative(&backend, root, Path::new("/repo/a\\b.txt")),
            Some((b"a\\b.txt".to_vec(), false))
        );
        assert_eq!(
            relative(&backend, root, Path::new("/repo/sub/file")),
            Some((b"sub".to_vec(), true))
        );
        assert_eq!(
            relative(&backend, root, Path::new("/repo/sub/")),
            Some((b"sub".to_vec(), false))
        );
        assert_eq!(
            relative(&backend, root, Path::new("/repo-other/file")),
            None
        );
        assert_eq!(
            relative(&backend, Path::new("/"), Path::new("/root/file")),
            Some((b"root".to_vec(), true))
        );
    }
}
