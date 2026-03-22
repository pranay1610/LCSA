use serde::{Deserialize, Serialize};

use crate::signals::SignalType;
use crate::topology::Platform;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MobileClipboardModel {
    LegacyBackgroundReadable,
    ForegroundOrImeOnly,
    UserIntentGated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalDeliveryModel {
    SystemWide,
    AppLocalOnly,
    ForegroundOnly,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobilePolicy {
    pub platform: Platform,
    pub major_version: u32,
    pub clipboard_model: MobileClipboardModel,
    pub selection_model: SignalDeliveryModel,
    pub focus_model: SignalDeliveryModel,
}

impl MobilePolicy {
    pub fn for_platform(platform: Platform, os_version: Option<&str>) -> Option<Self> {
        match platform {
            Platform::Android => {
                let major = parse_major_version(os_version)?;
                let clipboard_model = if major <= 9 {
                    MobileClipboardModel::LegacyBackgroundReadable
                } else {
                    MobileClipboardModel::ForegroundOrImeOnly
                };

                Some(Self {
                    platform,
                    major_version: major,
                    clipboard_model,
                    selection_model: SignalDeliveryModel::AppLocalOnly,
                    focus_model: SignalDeliveryModel::AppLocalOnly,
                })
            }
            Platform::Ios => {
                let major = parse_major_version(os_version)?;
                let clipboard_model = if major >= 16 {
                    MobileClipboardModel::UserIntentGated
                } else {
                    MobileClipboardModel::ForegroundOrImeOnly
                };

                Some(Self {
                    platform,
                    major_version: major,
                    clipboard_model,
                    selection_model: SignalDeliveryModel::AppLocalOnly,
                    focus_model: SignalDeliveryModel::AppLocalOnly,
                })
            }
            _ => None,
        }
    }

    pub fn signal_delivery(&self, signal_type: SignalType) -> SignalDeliveryModel {
        match signal_type {
            SignalType::Clipboard => match self.clipboard_model {
                MobileClipboardModel::LegacyBackgroundReadable => SignalDeliveryModel::SystemWide,
                MobileClipboardModel::ForegroundOrImeOnly => SignalDeliveryModel::ForegroundOnly,
                MobileClipboardModel::UserIntentGated => SignalDeliveryModel::AppLocalOnly,
            },
            SignalType::Selection => self.selection_model,
            SignalType::Focus => self.focus_model,
        }
    }
}

fn parse_major_version(version: Option<&str>) -> Option<u32> {
    let version = version?.trim();
    if version.is_empty() {
        return None;
    }

    let mut digits = String::new();
    for char in version.chars() {
        if char.is_ascii_digit() {
            digits.push(char);
        } else if !digits.is_empty() {
            break;
        }
    }

    if digits.is_empty() {
        None
    } else {
        digits.parse::<u32>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn android_legacy_policy_is_systemwide_for_clipboard() {
        let policy = MobilePolicy::for_platform(Platform::Android, Some("9")).expect("policy");
        assert_eq!(
            policy.clipboard_model,
            MobileClipboardModel::LegacyBackgroundReadable
        );
        assert_eq!(
            policy.signal_delivery(SignalType::Clipboard),
            SignalDeliveryModel::SystemWide
        );
    }

    #[test]
    fn android_modern_policy_limits_clipboard_scope() {
        let policy = MobilePolicy::for_platform(Platform::Android, Some("14.0.0")).expect("policy");
        assert_eq!(
            policy.clipboard_model,
            MobileClipboardModel::ForegroundOrImeOnly
        );
        assert_eq!(
            policy.signal_delivery(SignalType::Clipboard),
            SignalDeliveryModel::ForegroundOnly
        );
    }

    #[test]
    fn ios_16_and_newer_is_user_intent_gated() {
        let policy = MobilePolicy::for_platform(Platform::Ios, Some("16.6")).expect("policy");
        assert_eq!(
            policy.clipboard_model,
            MobileClipboardModel::UserIntentGated
        );
        assert_eq!(
            policy.signal_delivery(SignalType::Clipboard),
            SignalDeliveryModel::AppLocalOnly
        );
    }

    #[test]
    fn ios_15_is_foreground_only_for_clipboard() {
        let policy = MobilePolicy::for_platform(Platform::Ios, Some("15")).expect("policy");
        assert_eq!(
            policy.clipboard_model,
            MobileClipboardModel::ForegroundOrImeOnly
        );
        assert_eq!(
            policy.signal_delivery(SignalType::Clipboard),
            SignalDeliveryModel::ForegroundOnly
        );
    }
}
