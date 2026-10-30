use super::*;

#[test]
fn ssh_directories_stay_posix_even_on_windows_clients() {
    let body = b"file://remote.invalid/home/a%20b/project";
    assert_eq!(folder::folder_on(body, false), None);
    assert_eq!(folder::folder_on(body, true), None);
    assert_eq!(
        folder::remote(body),
        Some(PathBuf::from("/home/a b/project"))
    );
    assert_eq!(
        folder::remote(b"file://remote.invalid/root/a%5Cb"),
        Some(PathBuf::from("/root/a\\b"))
    );
    assert_eq!(folder::remote(b"file://remote.invalid/%FF"), None);
    assert_eq!(folder::remote(b"https://remote.invalid/root"), None);
}

#[test]
fn foreign_directory_reports_coexist_with_named_prompts_and_graphics() {
    let (events, mut heard) = futures::channel::mpsc::unbounded();
    let (mut kitty, mut titles, mut named, mut last) =
        (Kitty::default(), Titles::default(), false, None);
    let bytes = b"\x1b]133;A;aid=elyterminal\x07\x1b_Ga=q,i=1;AAAA\x1b\\\x1b]7;file://remote.invalid/home/project\x07";
    Osc::default().feed(bytes, &mut |found| {
        report(
            found,
            &events,
            10,
            &mut last,
            (&mut kitty, &mut titles, &mut named),
            &|| Some(10),
            &|_, _| false,
        );
    });
    assert!(matches!(
        heard.try_recv(),
        Ok(PtyEvent::Mark(Mark::Prompt(_), Holder::Shell))
    ));
    assert!(
        matches!(heard.try_recv(), Ok(PtyEvent::RemoteFolder(path)) if path == std::path::Path::new("/home/project"))
    );
    assert!(heard.try_recv().is_err());
    assert!(
        last.is_none(),
        "remote paths never become local directories"
    );
}
