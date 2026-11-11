use serde::{Deserialize, Serialize};

/// The interface's language, by the tag Ely's `I18n` names it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[default]
    #[serde(rename = "en-US")]
    English,
    #[serde(rename = "zh-CN")]
    SimplifiedChinese,
}

impl Language {
    pub fn tag(self) -> &'static str {
        match self {
            Self::English => "en-US",
            Self::SimplifiedChinese => "zh-CN",
        }
    }
}
