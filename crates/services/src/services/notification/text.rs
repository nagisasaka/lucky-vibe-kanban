use super::NotificationEvent;
use crate::services::config::UiLanguage;

/// Owned prose only. Workspace, branch, executor and tool identities remain raw.
pub enum OwnedNotification<'a> {
    WorkspaceFinished {
        workspace: &'a str,
        branch: &'a str,
        executor: Option<&'a str>,
        success: bool,
    },
    Questions {
        workspace: Option<&'a str>,
        count: usize,
    },
    Approval {
        workspace: Option<&'a str>,
        tool: &'a str,
    },
}

impl OwnedNotification<'_> {
    pub fn event(&self) -> NotificationEvent {
        match self {
            Self::WorkspaceFinished { success: true, .. } => NotificationEvent::Success,
            Self::WorkspaceFinished { success: false, .. } => NotificationEvent::Failure,
            Self::Questions { .. } => NotificationEvent::Question,
            Self::Approval { .. } => NotificationEvent::Approval,
        }
    }

    pub fn render(&self, language: UiLanguage) -> (String, String) {
        // The service resolves saved Browser hints before rendering. Unresolved
        // Browser and incomplete locales retain the explicit English fallback.
        let japanese = matches!(language, UiLanguage::Ja);
        let unknown = if japanese {
            "不明なワークスペース"
        } else {
            "Unknown workspace"
        };
        match self {
            Self::WorkspaceFinished {
                workspace,
                branch,
                executor,
                success,
            } => {
                let executor = executor.unwrap_or(if japanese { "不明" } else { "Unknown" });
                if japanese {
                    (
                        format!("ワークスペースの実行終了: {workspace}"),
                        format!(
                            "{}「{workspace}」の実行が{}\nブランチ: {branch}\nエージェント: {executor}",
                            if *success { "✅ " } else { "❌ " },
                            if *success {
                                "正常に終了しました"
                            } else {
                                "失敗しました"
                            }
                        ),
                    )
                } else {
                    (
                        format!("Workspace Complete: {workspace}"),
                        format!(
                            "{} '{workspace}' {}\nBranch: {branch}\nExecutor: {executor}",
                            if *success { "✅" } else { "❌" },
                            if *success {
                                "completed successfully"
                            } else {
                                "execution failed"
                            }
                        ),
                    )
                }
            }
            Self::Questions { workspace, count } => {
                let workspace = workspace.unwrap_or(unknown);
                if japanese {
                    (
                        format!("質問があります: {workspace}"),
                        format!("{count}件の質問への回答が必要です"),
                    )
                } else {
                    (
                        format!("Question Asked: {workspace}"),
                        if *count == 1 {
                            "1 question requires an answer".to_string()
                        } else {
                            format!("{count} questions require answers")
                        },
                    )
                }
            }
            Self::Approval { workspace, tool } => {
                let workspace = workspace.unwrap_or(unknown);
                if japanese {
                    (
                        format!("承認が必要です: {workspace}"),
                        format!("ツール「{tool}」の承認が必要です"),
                    )
                } else {
                    (
                        format!("Approval Needed: {workspace}"),
                        format!("Tool '{tool}' requires approval"),
                    )
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_notifications_preserve_identities_and_plural_meaning() {
        let content = OwnedNotification::WorkspaceFinished {
            workspace: "Raw workspace 原文",
            branch: "feature/原文",
            executor: Some("RAW_EXECUTOR"),
            success: false,
        };
        for language in [UiLanguage::En, UiLanguage::Ja] {
            let (title, body) = content.render(language);
            assert!(title.contains("Raw workspace 原文"));
            assert!(body.contains("feature/原文"));
            assert!(body.contains("RAW_EXECUTOR"));
        }
        assert!(content.render(UiLanguage::Ja).1.contains("失敗しました"));
        let one = OwnedNotification::Questions {
            workspace: None,
            count: 1,
        };
        assert_eq!(
            one.render(UiLanguage::Ja),
            (
                "質問があります: 不明なワークスペース".into(),
                "1件の質問への回答が必要です".into()
            )
        );
        assert_eq!(
            one.render(UiLanguage::En).1,
            "1 question requires an answer"
        );
        let multiple = OwnedNotification::Questions {
            workspace: Some("原文"),
            count: 2,
        };
        assert_eq!(
            multiple.render(UiLanguage::En).1,
            "2 questions require answers"
        );
        let approval = OwnedNotification::Approval {
            workspace: None,
            tool: "Raw tool 原文",
        };
        assert_eq!(
            approval.render(UiLanguage::Ja).1,
            "ツール「Raw tool 原文」の承認が必要です"
        );
        for language in [
            UiLanguage::Browser,
            UiLanguage::Fr,
            UiLanguage::Es,
            UiLanguage::Ko,
            UiLanguage::ZhHans,
            UiLanguage::ZhHant,
        ] {
            assert_eq!(approval.render(language), approval.render(UiLanguage::En));
        }
    }
}
