//! `.pclm` 을 이 exe 에 잇는다(C# `Pclm.App/FileAssociation.cs`) — 받은 제출본을 두 번 누르면 이 프로그램을 몰라도 열람 창이
//! 뜬다.
//!
//! 배포물은 exe 하나라 설치기가 없다. 그래서 창을 켤 때마다 적는다 — exe 를 새 판으로 바꾸거나 옮기면 옛 자리를 가리키는
//! 연결이 남는다. 관리자 권한이 들지 않게 `HKCU\Software\Classes` 에만 적는다.
//!
//! **업무 홈에서만 적는다**([`refresh_for`]). 연결은 컴퓨터에 하나뿐이라, `--home` 으로 띄운 시험 창이 고쳐 적으면 사람이
//! 두 번 누른 파일이 시험 홈의 창으로 간다. 등록부의 손은 [`Registry`] 로 갈아 끼울 수 있다 — 시험은 진짜 HKCU 에 닿지 않는다.

use std::io;
use std::path::Path;

use pclm_home::{Home, dotnet};

const EXTENSION: &str = ".pclm";
const PROG_ID: &str = "Pclm.Workfile";

/// `HKCU` 아래에 하는 일. 키 이름은 `Software\Classes\…` 꼴이다.
pub trait Registry {
    /// `OpenSubKey(key)?.GetValue("") as string` — 키나 값이 없거나 글이 아니면 `None`.
    fn default_value(&self, key: &str) -> io::Result<Option<String>>;
    /// `CreateSubKey(key).SetValue(name, value)` — 글 값(`REG_SZ`). 빈 이름은 기본값이다.
    fn set_string(&self, key: &str, name: &str, value: &str) -> io::Result<()>;
    /// `SetValue(name, Array.Empty<byte>(), RegistryValueKind.None)` — 꼴 없는 빈 값.
    fn set_empty(&self, key: &str, name: &str) -> io::Result<()>;
    /// `SHChangeNotify(SHCNE_ASSOCCHANGED)` — 알리지 않으면 탐색기가 다시 켜질 때까지 옛 연결과 아이콘을 쥐고 있다.
    fn notify_changed(&self);
}

/// 창이 켜질 때 부른다 — 업무 홈일 때만 [`refresh`] 한다(C# `if (home.IsDefault) FileAssociation.Refresh()`).
pub fn refresh_for(registry: &dyn Registry, home: &Home, exe: Option<&Path>, temp: &Path) {
    if home.is_default() {
        refresh(registry, exe, temp);
    }
}

/// C# `FileAssociation.Refresh`. 창을 막을 일이 아니라 실패는 삼킨다 — 적지 못했으면 두 번 누른 파일이 연결 프로그램을 물을
/// 뿐이다.
pub fn refresh(registry: &dyn Registry, exe: Option<&Path>, temp: &Path) {
    let Some(exe) = exe else {
        return;
    };
    // 압축 파일 안에서 곧바로 켜면 탐색기가 임시 폴더에 풀어 띄운다. 그 자리는 곧 지워지니 적지 않는다.
    if let (Ok(full), Ok(temp)) = (dotnet::full_path(exe), dotnet::full_path(temp)) {
        let full = dotnet::to_upper_invariant(&full.to_string_lossy());
        if full.starts_with(&dotnet::to_upper_invariant(&temp.to_string_lossy())) {
            return;
        }
    }
    let _ = write(registry, exe);
}

fn write(registry: &dyn Registry, exe: &Path) -> io::Result<()> {
    let exe_text = exe.to_string_lossy();
    let command = format!("\"{exe_text}\" \"%1\"");
    let classes = r"Software\Classes";
    let current = registry.default_value(&format!(r"{classes}\{PROG_ID}\shell\open\command"))?;
    if current.as_deref() == Some(command.as_str()) && has_default(registry)? {
        return Ok(());
    }

    let extension = format!(r"{classes}\{EXTENSION}");
    registry.set_string(&extension, "", PROG_ID)?;
    registry.set_empty(&format!(r"{extension}\OpenWithProgids"), PROG_ID)?;

    let prog = format!(r"{classes}\{PROG_ID}");
    registry.set_string(&prog, "", "계약 목록 자료")?;
    registry.set_string(
        &format!(r"{prog}\DefaultIcon"),
        "",
        &format!("\"{exe_text}\",0"),
    )?;
    registry.set_string(&format!(r"{prog}\shell\open\command"), "", &command)?;

    // 「연결 프로그램」 목록에 exe 파일 이름 대신 제 이름으로 서게 한다.
    let file_name = exe
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let application = format!(r"{classes}\Applications\{file_name}");
    registry.set_string(&application, "FriendlyAppName", "계약 목록")?;
    registry.set_string(&format!(r"{application}\SupportedTypes"), EXTENSION, "")?;
    registry.set_string(&format!(r"{application}\shell\open\command"), "", &command)?;

    registry.notify_changed();
    Ok(())
}

/// 확장자가 우리 ProgId 를 기본으로 가리키는가. 다른 프로그램이 그 자리를 가져갔으면 되찾는다.
fn has_default(registry: &dyn Registry) -> io::Result<bool> {
    Ok(registry
        .default_value(&format!(r"Software\Classes\{EXTENSION}"))?
        .as_deref()
        == Some(PROG_ID))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    /// 적은 것을 기억하는 가짜 등록부.
    #[derive(Default)]
    struct Fake {
        values: RefCell<BTreeMap<(String, String), Option<String>>>,
        notified: RefCell<usize>,
    }

    impl Registry for Fake {
        fn default_value(&self, key: &str) -> io::Result<Option<String>> {
            Ok(self
                .values
                .borrow()
                .get(&(key.into(), String::new()))
                .cloned()
                .flatten())
        }
        fn set_string(&self, key: &str, name: &str, value: &str) -> io::Result<()> {
            self.values
                .borrow_mut()
                .insert((key.into(), name.into()), Some(value.into()));
            Ok(())
        }
        fn set_empty(&self, key: &str, name: &str) -> io::Result<()> {
            self.values
                .borrow_mut()
                .insert((key.into(), name.into()), None);
            Ok(())
        }
        fn notify_changed(&self) {
            *self.notified.borrow_mut() += 1;
        }
    }

    #[test]
    fn 처음에는_다_적고_같으면_다시_적지_않는다() {
        let fake = Fake::default();
        let exe = Path::new(r"C:\앱\계약목록.exe");
        refresh(&fake, Some(exe), Path::new(r"C:\임시"));
        let command = "\"C:\\앱\\계약목록.exe\" \"%1\"".to_string();
        let values = fake.values.borrow().clone();
        assert_eq!(
            values[&(r"Software\Classes\.pclm".into(), "".into())],
            Some("Pclm.Workfile".into())
        );
        assert_eq!(
            values[&(
                r"Software\Classes\.pclm\OpenWithProgids".into(),
                "Pclm.Workfile".into()
            )],
            None
        );
        assert_eq!(
            values[&(r"Software\Classes\Pclm.Workfile".into(), "".into())],
            Some("계약 목록 자료".into())
        );
        assert_eq!(
            values[&(
                r"Software\Classes\Pclm.Workfile\DefaultIcon".into(),
                "".into()
            )],
            Some("\"C:\\앱\\계약목록.exe\",0".into())
        );
        assert_eq!(
            values[&(
                r"Software\Classes\Pclm.Workfile\shell\open\command".into(),
                "".into()
            )],
            Some(command.clone())
        );
        assert_eq!(
            values[&(
                r"Software\Classes\Applications\계약목록.exe".into(),
                "FriendlyAppName".into()
            )],
            Some("계약 목록".into())
        );
        assert_eq!(
            values[&(
                r"Software\Classes\Applications\계약목록.exe\SupportedTypes".into(),
                ".pclm".into()
            )],
            Some("".into())
        );
        assert_eq!(values.len(), 8);
        assert_eq!(*fake.notified.borrow(), 1);

        refresh(&fake, Some(exe), Path::new(r"C:\임시"));
        assert_eq!(*fake.notified.borrow(), 1, "같은 자리면 다시 적지 않는다");

        // 다른 프로그램이 확장자를 가져갔으면 되찾는다.
        fake.set_string(r"Software\Classes\.pclm", "", "남의.ProgId")
            .unwrap();
        refresh(&fake, Some(exe), Path::new(r"C:\임시"));
        assert_eq!(*fake.notified.borrow(), 2);
    }

    #[test]
    fn 임시_폴더에서_켰거나_시험_홈이면_적지_않는다() {
        let fake = Fake::default();
        refresh(
            &fake,
            Some(Path::new(r"C:\임시\풀린\계약목록.exe")),
            Path::new(r"c:\임시\"),
        );
        assert!(fake.values.borrow().is_empty());

        let home = Home::new(std::env::temp_dir().join("pclm-desktop-시험-홈")).unwrap();
        refresh_for(
            &fake,
            &home,
            Some(Path::new(r"C:\앱\계약목록.exe")),
            Path::new(r"C:\임시"),
        );
        assert!(fake.values.borrow().is_empty());
    }
}
