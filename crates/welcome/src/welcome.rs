mod base_keymap_picker;
mod base_keymap_setting;
mod multibuffer_hint;

use client::{telemetry::Telemetry, TelemetrySettings};
use db::kvp::KEY_VALUE_STORE;
use gpui::{Window, ModelContext, Model, 
    actions, svg, Action, AppContext, EventEmitter, FocusHandle, FocusableView, InteractiveElement,
    ParentElement, Render, Styled, Subscription, Task,   VisualContext, WeakView,
    
};
use settings::{Settings, SettingsStore};
use std::sync::Arc;
use ui::{prelude::*, CheckboxWithLabel, Tooltip};
use vim_mode_setting::VimModeSetting;
use workspace::{
    dock::DockPosition,
    item::{Item, ItemEvent},
    open_new, AppState, Welcome, Workspace, WorkspaceId,
};

pub use base_keymap_setting::BaseKeymap;
pub use multibuffer_hint::*;

actions!(welcome, [ResetHints]);

pub const FIRST_OPEN: &str = "first_open";
pub const DOCS_URL: &str = "https://zed.dev/docs/";
const BOOK_ONBOARDING: &str = "https://dub.sh/zed-onboarding";

pub fn init(cx: &mut AppContext) {
    BaseKeymap::register(cx);

    cx.observe_new_views(|workspace: &mut Workspace, _cx| {
        workspace.register_action(|workspace, _: &Welcome, window, cx| {
            let welcome_page = WelcomePage::new(workspace, window, cx);
            workspace.add_item_to_active_pane(Box::new(welcome_page), None, true, window, cx)
        });
        workspace
            .register_action(|_workspace, _: &ResetHints, window, cx| MultibufferHint::set_count(0, cx));
    })
    .detach();

    base_keymap_picker::init(cx);
}

pub fn show_welcome_view(
    app_state: Arc<AppState>,
    cx: &mut AppContext,
) -> Task<anyhow::Result<()>> {
    open_new(Default::default(), app_state, cx, |workspace, window, cx| {
        workspace.toggle_dock(DockPosition::Left, window, cx);
        let welcome_page = WelcomePage::new(workspace, window, cx);
        workspace.add_item_to_center(Box::new(welcome_page.clone()), window, cx);
        window.focus_view(&welcome_page, cx);
        cx.notify();

        db::write_and_log(cx, || {
            KEY_VALUE_STORE.write_kvp(FIRST_OPEN.to_string(), "false".to_string())
        });
    })
}

pub struct WelcomePage {
    workspace: WeakView<Workspace>,
    focus_handle: FocusHandle,
    telemetry: Arc<Telemetry>,
    _settings_subscription: Subscription,
}

impl Render for WelcomePage {
    fn render(&mut self, window: &mut Window, cx: &mut ModelContext<Self>) -> impl IntoElement {
        h_flex()
            .size_full()
            .bg(cx.theme().colors().editor_background)
            .key_context("Welcome")
            .track_focus(&self.focus_handle(cx))
            .child(
                v_flex()
                    .gap_8()
                    .mx_auto()
                    .child(
                        v_flex()
                            .w_full()
                            .child(
                                svg()
                                    .path("icons/logo_96.svg")
                                    .text_color(cx.theme().colors().icon_disabled)
                                    .w(px(40.))
                                    .h(px(40.))
                                    .mx_auto()
                                    .mb_4(),
                            )
                            .child(
                                h_flex()
                                    .w_full()
                                    .justify_center()
                                    .child(Headline::new("Welcome to Zed")),
                            )
                            .child(
                                h_flex().w_full().justify_center().child(
                                    Label::new("The editor for what's next")
                                        .color(Color::Muted)
                                        .italic(true),
                                ),
                            ),
                    )
                    .child(
                        h_flex()
                            .items_start()
                            .gap_8()
                            .child(
                                v_flex()
                                    .gap_2()
                                    .pr_8()
                                    .border_r_1()
                                    .border_color(cx.theme().colors().border_variant)
                                    .child(
                                        self.section_label(window, cx).child(
                                            Label::new("Get Started")
                                                .size(LabelSize::XSmall)
                                                .color(Color::Muted),
                                        ),
                                    )
                                    .child(
                                        Button::new("choose-theme", "Choose a Theme")
                                            .icon(IconName::SwatchBook)
                                            .icon_size(IconSize::XSmall)
                                            .icon_color(Color::Muted)
                                            .icon_position(IconPosition::Start)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.telemetry.report_app_event(
                                                    "welcome page: change theme".to_string(),
                                                );
                                                this.workspace
                                                    .update(cx, |_workspace, cx| {
                                                        window.dispatch_action(zed_actions::theme_selector::Toggle::default().boxed_clone(), cx);
                                                    })
                                                    .ok();
                                            })),
                                    )
                                    .child(
                                        Button::new("choose-keymap", "Choose a Keymap")
                                            .icon(IconName::Keyboard)
                                            .icon_size(IconSize::XSmall)
                                            .icon_color(Color::Muted)
                                            .icon_position(IconPosition::Start)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.telemetry.report_app_event(
                                                    "welcome page: change keymap".to_string(),
                                                );
                                                this.workspace
                                                    .update(cx, |workspace, cx| {
                                                        base_keymap_picker::toggle(
                                                            workspace,
                                                            &Default::default(),
                                                            window, cx,
                                                        )
                                                    })
                                                    .ok();
                                            })),
                                    )
                                    .child(
                                        Button::new(
                                            "sign-in-to-copilot",
                                            "Sign in to GitHub Copilot",
                                        )
                                        .icon(IconName::Copilot)
                                        .icon_size(IconSize::XSmall)
                                        .icon_color(Color::Muted)
                                        .icon_position(IconPosition::Start)
                                        .on_click(
                                            cx.listener(|this, _, window, cx| {
                                                this.telemetry.report_app_event(
                                                    "welcome page: sign in to copilot".to_string(),
                                                );
                                                copilot::initiate_sign_in(window, cx);
                                            }),
                                        ),
                                    )
                                    .child(
                                        Button::new("edit settings", "Edit Settings")
                                            .icon(IconName::Settings)
                                            .icon_size(IconSize::XSmall)
                                            .icon_color(Color::Muted)
                                            .icon_position(IconPosition::Start)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.telemetry.report_app_event(
                                                    "welcome page: edit settings".to_string(),
                                                );
                                                window.dispatch_action(Box::new(
                                                    zed_actions::OpenSettings,
                                                ), cx);
                                            })),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .gap_2()
                                    .child(
                                        self.section_label(window, cx).child(
                                            Label::new("Resources")
                                                .size(LabelSize::XSmall)
                                                .color(Color::Muted),
                                        ),
                                    )
                                    .when(cfg!(target_os = "macos"), |el| {
                                        el.child(
                                            Button::new("install-cli", "Install the CLI")
                                                .icon(IconName::Terminal)
                                                .icon_size(IconSize::XSmall)
                                                .icon_color(Color::Muted)
                                                .icon_position(IconPosition::Start)
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.telemetry.report_app_event(
                                                        "welcome page: install cli".to_string(),
                                                    );
                                                    cx.app_mut()
                                                        .spawn(|cx| async move {
                                                            install_cli::install_cli(&cx).await
                                                        })
                                                        .detach_and_log_err(cx);
                                                })),
                                        )
                                    })
                                    .child(
                                        Button::new("view-docs", "View Documentation")
                                            .icon(IconName::FileCode)
                                            .icon_size(IconSize::XSmall)
                                            .icon_color(Color::Muted)
                                            .icon_position(IconPosition::Start)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.telemetry.report_app_event(
                                                    "welcome page: view docs".to_string(),
                                                );
                                                cx.open_url(DOCS_URL);
                                            })),
                                    )
                                    .child(
                                        Button::new("explore-extensions", "Explore Extensions")
                                            .icon(IconName::Blocks)
                                            .icon_size(IconSize::XSmall)
                                            .icon_color(Color::Muted)
                                            .icon_position(IconPosition::Start)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.telemetry.report_app_event(
                                                    "welcome page: open extensions".to_string(),
                                                );
                                                window.dispatch_action(Box::new(
                                                    zed_actions::Extensions,
                                                ), cx);
                                            })),
                                    )
                                    .child(
                                        Button::new("book-onboarding", "Book Onboarding")
                                            .icon(IconName::PhoneIncoming)
                                            .icon_size(IconSize::XSmall)
                                            .icon_color(Color::Muted)
                                            .icon_position(IconPosition::Start)
                                            .on_click(cx.listener(|_, _, window, cx| {
                                                cx.open_url(BOOK_ONBOARDING);
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        v_group()
                            .gap_2()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .child(CheckboxWithLabel::new(
                                        "enable-vim",
                                        Label::new("Enable Vim Mode"),
                                        if VimModeSetting::get_global(cx).0 {
                                            ui::ToggleState::Selected
                                        } else {
                                            ui::ToggleState::Unselected
                                        },
                                        cx.listener(move |this, selection, window, cx| {
                                            this.telemetry
                                                .report_app_event("welcome page: toggle vim".to_string());
                                            this.update_settings::<VimModeSetting>(
                                                selection,
                                                window, cx,
                                                |setting, value| *setting = Some(value),
                                            );
                                        }),
                                    ))
                                    .child(
                                        IconButton::new("vim-mode", IconName::Info)
                                            .icon_size(IconSize::XSmall)
                                            .icon_color(Color::Muted)
                                            .tooltip(|window, cx| Tooltip::text("You can also toggle Vim Mode via the command palette or Editor Controls menu.", window, cx)),
                                    )
                            )
                            .child(CheckboxWithLabel::new(
                                "enable-crash",
                                Label::new("Send Crash Reports"),
                                if TelemetrySettings::get_global(cx).diagnostics {
                                    ui::ToggleState::Selected
                                } else {
                                    ui::ToggleState::Unselected
                                },
                                cx.listener(move |this, selection, window, cx| {
                                    this.telemetry.report_app_event(
                                        "welcome page: toggle diagnostic telemetry".to_string(),
                                    );
                                    this.update_settings::<TelemetrySettings>(selection, window, cx, {
                                        move |settings, value| {
                                            settings.diagnostics = Some(value);

                                            telemetry::event!("Settings Changed", setting = "diagnostic telemetry", value);
                                        }
                                    });
                                }),
                            ))
                            .child(CheckboxWithLabel::new(
                                "enable-telemetry",
                                Label::new("Send Telemetry"),
                                if TelemetrySettings::get_global(cx).metrics {
                                    ui::ToggleState::Selected
                                } else {
                                    ui::ToggleState::Unselected
                                },
                                cx.listener(move |this, selection, window, cx| {
                                    this.telemetry.report_app_event(
                                        "welcome page: toggle metric telemetry".to_string(),
                                    );
                                    this.update_settings::<TelemetrySettings>(selection, window, cx, {
                                        move |settings, value| {
                                            settings.metrics = Some(value);
                                            telemetry::event!("Settings Changed", setting = "metric telemetry", value);
                                        }
                                    });
                                }),
                            )),
                    ),
            )
    }
}

impl WelcomePage {
    pub fn new(workspace: &Workspace, window: &mut Window, cx: &mut ModelContext<Workspace>) -> Model<Self> {
        let this = window.new_view(cx, |cx| {
            cx.subscribe_in(window, |this: &mut Self, _, _, _| {
                this.telemetry
                    .report_app_event("welcome page: close".to_string());
            })
            .detach();

            WelcomePage {
                focus_handle: cx.focus_handle(),
                workspace: workspace.weak_handle(),
                telemetry: workspace.client().telemetry().clone(),
                _settings_subscription: cx
                    .observe_global_in::<SettingsStore>(window, move |_, window, cx| cx.notify()),
            }
        });

        this
    }

    fn section_label(&self, window: &mut Window, cx: &mut AppContext) -> Div {
        div()
            .pl_1()
            .font_buffer(window, cx)
            .text_color(Color::Muted.color(window, cx))
    }

    fn update_settings<T: Settings>(
        &mut self,
        selection: &ToggleState,
        window: &mut Window, cx: &mut ModelContext<Self>,
        callback: impl 'static + Send + Fn(&mut T::FileContent, bool),
    ) {
        if let Some(workspace) = self.workspace.upgrade() {
            let fs = workspace.read(cx).app_state().fs.clone();
            let selection = *selection;
            settings::update_settings_file::<T>(fs, cx, move |settings, _| {
                let value = match selection {
                    ToggleState::Unselected => false,
                    ToggleState::Selected => true,
                    _ => return,
                };

                callback(settings, value)
            });
        }
    }
}

impl EventEmitter<ItemEvent> for WelcomePage {}

impl FocusableView for WelcomePage {
    fn focus_handle(&self, _: &AppContext) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }
}

impl Item for WelcomePage {
    type Event = ItemEvent;

    fn tab_content_text(&self, _window: &mut Window, _cx: &mut AppContext) -> Option<SharedString> {
        Some("Welcome".into())
    }

    fn telemetry_event_text(&self) -> Option<&'static str> {
        Some("welcome page")
    }

    fn show_toolbar(&self) -> bool {
        false
    }

    fn clone_on_split(
        &self,
        _workspace_id: Option<WorkspaceId>,
        window: &mut Window, cx: &mut ModelContext<Self>,
    ) -> Option<Model<Self>> {
        Some(window.new_view(cx, |cx| WelcomePage {
            focus_handle: cx.focus_handle(),
            workspace: self.workspace.clone(),
            telemetry: self.telemetry.clone(),
            _settings_subscription: cx.observe_global_in::<SettingsStore>(window, move |_, window, cx| cx.notify()),
        }))
    }

    fn to_item_events(event: &Self::Event, mut f: impl FnMut(workspace::item::ItemEvent)) {
        f(*event)
    }
}
