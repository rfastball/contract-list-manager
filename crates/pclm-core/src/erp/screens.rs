//! 수집하는 나라장터 화면(C# `Erp/Screens.cs`) — 화면의 메뉴 번호(다섯 자리)로 가린 허용 목록이다(ADR-037).
//!
//! 목록은 따로 두지 않는다: 매핑에서 `screen` 을 단 프로필들이 그것이고, 같은 프로필이 그 화면의 고유키를 들고 있다.
//! 받는 것은 번호가 목록에 있고 **그리고** 그 프로필의 고유키가 읽힐 때뿐이다 — 뒤의 것은 [`mapping::map`] 이 본다.
//! 번호를 읽지 못한 화면·파일도 받지 않는다.

use serde_json::Value;

use super::mapping::{self, MappingProfile, MappingSet};
use crate::error::{Error, Result};

/// `Screens.Supported` — 수집하는 화면들, 매핑에서 화면을 단 프로필.
pub fn supported(mapping: &MappingSet) -> impl Iterator<Item = &MappingProfile> {
    mapping.profiles_iter().filter(|p| p.screen.is_some())
}

/// `Screens.IsCode` — 메뉴 번호의 모양(`\A\d{5}\z`, .NET 의 `\d` 는 유니코드 십진 숫자)인가.
pub fn is_code(value: Option<&str>) -> bool {
    value.is_some_and(|v| v.chars().count() == 5 && v.chars().all(mapping::dotnet_digit))
}

/// `Screens.ProfileFor` — 그 화면을 읽는 프로필. 목록에 없으면 `None`.
pub fn profile_for<'a>(mapping: &'a MappingSet, code: Option<&str>) -> Option<&'a MappingProfile> {
    if !is_code(code) {
        return None;
    }
    supported(mapping).find(|p| p.screen.as_ref().and_then(|s| s.code.as_deref()) == code)
}

/// `Screens.Leaf` — ERP JSON 의 `pointInfo` 가 적은 메뉴 경로에서 그 화면의 번호. 비지 않은 가장 깊은 단(depth3 →
/// depth2 → depth1). 없거나 모양이 틀리면 `None`.
pub fn leaf(point_info: &Value) -> Option<String> {
    let point = point_info.as_object()?;
    for name in ["depth3", "depth2", "depth1"] {
        let Some(Value::String(value)) = point.get(name) else {
            continue;
        };
        let text = value.trim();
        if text.is_empty() {
            continue;
        }
        return is_code(Some(text)).then(|| text.to_string());
    }
    None
}

/// `Mirror.KindName` — 사람이 읽을 종류 이름.
pub fn kind_name(entity_type: &str) -> &'static str {
    match entity_type {
        "request" => "접수",
        "notice" => "공고",
        _ => "계약",
    }
}

/// `Screens.Check` — 그 화면의 자료를 이 프로필로 받아도 되는가. 번호를 모르거나, 목록에 없거나, 그 화면의 프로필이
/// 아니면 사람이 읽을 까닭을 담아 거절한다. `live` 는 확장이 읽은 실제 화면인가 — 파일이면 프로필을 고쳐 고르라고 말한다.
pub fn check(mapping: &MappingSet, code: Option<&str>, profile: &str, live: bool) -> Result<()> {
    if !is_code(code) {
        return Err(Error::invalid(if live {
            "화면 번호를 읽지 못했습니다. 접수·공고·계약 상세 화면에서 다시 읽어 주세요."
        } else {
            "이 JSON 에는 화면 번호(depth)가 없어 받지 않습니다."
        }));
    }
    let code = code.unwrap_or_default();
    let Some(expected) = profile_for(mapping, Some(code)) else {
        return Err(Error::invalid(format!(
            "메뉴 {code} 화면은 수집하지 않습니다."
        )));
    };
    if expected.id() == profile {
        return Ok(());
    }
    let name = kind_name(expected.entity_type());
    Err(Error::invalid(if live {
        format!("{name} 화면(메뉴 {code})인데 다른 프로필로 읽혔습니다. 화면을 다시 읽어 주세요.")
    } else {
        format!(
            "{name} 화면(메뉴 {code})의 JSON 입니다. {name} 프로필({})로 여세요.",
            expected.id()
        )
    }))
}
