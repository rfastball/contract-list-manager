//! THIRD-PARTY-NOTICES.txt 가 배포 exe 에 실제로 실리는 것과 같은가 — `== crate` 는 `cargo tree`, `== npm` 은
//! `package-lock.json` 의 운영 의존, `== font` 는 `frontend/fonts` 와 견준다(C# ThirdPartyNoticesTests 의 Rust 짝). 의존을
//! 더하거나 판을 올리고 고지를 고치지 않으면 여기서 붉어진다. 수를 박아 두지 않는다 — 판이 오르면 붉어지는 것이 의도다.

use std::path::{Path, PathBuf};

use pclm_desktop::notices;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

#[test]
fn 실리는_크레이트가_이름_판_라이선스로_모두_적혀_있고_적힌_것은_모두_실린다() {
    let cargo = PathBuf::from(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    let shipped = notices::shipped(&cargo, &root()).expect("cargo tree");
    assert!(
        !shipped.is_empty(),
        "cargo tree 가 실리는 크레이트를 하나도 내지 않았습니다."
    );

    let text = std::fs::read_to_string(root().join("THIRD-PARTY-NOTICES.txt")).expect("고지 파일");
    let written = notices::written(&text);

    let missing: Vec<String> = shipped
        .difference(&written)
        .map(|c| format!("{} {} — {}", c.name, c.version, c.license))
        .collect();
    let stale: Vec<String> = written
        .difference(&shipped)
        .map(|c| format!("{} {} — {}", c.name, c.version, c.license))
        .collect();
    assert!(
        missing.is_empty() && stale.is_empty(),
        "THIRD-PARTY-NOTICES.txt 의 crate 항목이 실제와 어긋납니다.\n  고지에 없음: {}\n  더는 실리지 않음: {}\n\
         ./scripts/generate-crate-notices.ps1 로 다시 지으세요.",
        missing.join(", "),
        stale.join(", ")
    );
}

#[test]
fn 크레이트가_가리키는_전문이_문서_끝에_있다() {
    let text = std::fs::read_to_string(root().join("THIRD-PARTY-NOTICES.txt")).expect("고지 파일");
    let end = text.rfind("\n[MIT]\n").expect("문서 끝의 [MIT] 전문");
    assert!(
        text[end..].contains("\n[Apache-2.0]\n"),
        "문서 끝의 [Apache-2.0] 전문"
    );
    for marker in ["문서 끝 [MIT] 참조", "문서 끝 [Apache-2.0] 참조"] {
        assert!(text.contains(marker), "{marker}");
    }
}

/// 고지의 머리줄 `== <갈래> <이름> <판> — <라이선스>` 중 그 갈래의 `이름 판`.
fn heads(text: &str, kind: &str) -> std::collections::BTreeSet<String> {
    let prefix = format!("== {kind} ");
    text.lines()
        .filter_map(|line| line.strip_prefix(&prefix))
        .filter_map(|rest| {
            let mut words = rest.split(' ');
            let (name, version, dash) = (words.next()?, words.next()?, words.next()?);
            (dash == "—").then(|| format!("{name} {version}"))
        })
        .collect()
}

fn same(
    shipped: &std::collections::BTreeSet<String>,
    written: &std::collections::BTreeSet<String>,
    kind: &str,
) {
    let missing: Vec<&String> = shipped.difference(written).collect();
    let stale: Vec<&String> = written.difference(shipped).collect();
    assert!(
        missing.is_empty() && stale.is_empty(),
        "THIRD-PARTY-NOTICES.txt 의 {kind} 항목이 실제와 어긋납니다.\n  고지에 없음: {missing:?}\n  더는 실리지 않음: {stale:?}\n\
         고지 파일을 함께 고치세요(이름·판·라이선스·저작권 줄)."
    );
}

/// C# `ThirdPartyNoticesTests.npm_운영_의존이_이름과_판으로_모두_적혀_있고_적힌_것은_모두_실린다` — 화면(exe 에 박힌다)에 실리는
/// npm 운영 의존은 `package-lock.json` 의 dev · devOptional 이 아닌 꾸러미다.
#[test]
fn npm_운영_의존이_이름과_판으로_모두_적혀_있고_적힌_것은_모두_실린다() {
    let lock: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("package-lock.json")).expect("package-lock.json"),
    )
    .unwrap();
    const PREFIX: &str = "node_modules/";
    let flag =
        |v: &serde_json::Value, name: &str| v.get(name) == Some(&serde_json::Value::Bool(true));
    let shipped = lock["packages"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(name, _)| name.starts_with(PREFIX))
        .filter(|(_, v)| !flag(v, "dev") && !flag(v, "devOptional"))
        .map(|(name, v)| {
            let at = name.rfind(PREFIX).unwrap() + PREFIX.len();
            format!("{} {}", &name[at..], v["version"].as_str().unwrap())
        })
        .collect();
    let text = std::fs::read_to_string(root().join("THIRD-PARTY-NOTICES.txt")).expect("고지 파일");
    same(&shipped, &heads(&text, "npm"), "npm");
}

/// C# `ThirdPartyNoticesTests.화면에_싣는_글꼴마다_고지가_있다` — `frontend/fonts/*.woff2` 와 `== font` 항목이 짝이다.
#[test]
fn 화면에_싣는_글꼴마다_고지가_있다() {
    let fonts = root().join("frontend").join("fonts");
    let shipped: std::collections::BTreeSet<String> = std::fs::read_dir(&fonts)
        .map(|entries| {
            entries
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .filter(|n| n.ends_with(".woff2"))
                .collect()
        })
        .unwrap_or_default();
    let text = std::fs::read_to_string(root().join("THIRD-PARTY-NOTICES.txt")).expect("고지 파일");
    let written: std::collections::BTreeSet<String> = heads(&text, "font")
        .into_iter()
        .map(|e| e.split(' ').next().unwrap().to_string())
        .collect();
    same(&shipped, &written, "font");
}
