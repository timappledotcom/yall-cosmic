// SPDX-License-Identifier: MPL-2.0

use crate::config::{Config, BlueskyConfig, MastodonConfig, NostrConfig};
use crate::fl;
use crate::social::{post_to_bluesky, post_to_mastodon, post_to_nostr, PostError};
use cosmic::app::context_drawer;
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::iced::alignment::{Horizontal, Vertical};
use cosmic::iced::{Alignment, Length, Subscription};
use cosmic::prelude::*;
use cosmic::widget::{self, icon, menu, nav_bar, text_input, button, checkbox, column, row, container, scrollable};
use cosmic::{cosmic_theme, theme};
use futures_util::SinkExt;
use std::collections::HashMap;

const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
const APP_ICON: &[u8] = include_bytes!("../resources/icons/hicolor/scalable/apps/icon.svg");
const MAX_POST_LENGTH: usize = 280;

/// The application model stores app-specific state used to describe its interface and
/// drive its logic.
pub struct AppModel {
    /// Application state which is managed by the COSMIC runtime.
    core: cosmic::Core,
    /// Display a context drawer with the designated page if defined.
    context_page: ContextPage,
    /// Contains items assigned to the nav bar panel.
    nav: nav_bar::Model,
    /// Key bindings for the application's menu bar.
    key_binds: HashMap<menu::KeyBind, MenuAction>,
    // Configuration data that persists between application runs.
    config: Config,
    // UI state
    post_text: String,
    posting_status: PostingStatus,
    // Settings editing state
    temp_bluesky: BlueskyConfig,
    temp_mastodon: MastodonConfig,
    temp_nostr: NostrConfig,
    new_relay: String,
}

#[derive(Debug, Clone, Default)]
pub enum PostingStatus {
    #[default]
    Idle,
    Posting,
    Success,
    Error(String),
}

/// Messages emitted by the application and its widgets.
#[derive(Debug, Clone)]
pub enum Message {
    OpenRepositoryUrl,
    SubscriptionChannel,
    ToggleContextPage(ContextPage),
    UpdateConfig(Config),
    LaunchUrl(String),
    // Post composition
    PostTextChanged(String),
    PostSubmit,
    PostResult(Result<(), PostError>),
    // Settings
    BlueskyEnabledChanged(bool),
    BlueskyHandleChanged(String),
    BlueskyPasswordChanged(String),
    MastodonEnabledChanged(bool),
    MastodonInstanceChanged(String),
    MastodonTokenChanged(String),
    NostrEnabledChanged(bool),
    NostrPrivateKeyChanged(String),
    NewRelayChanged(String),
    AddRelay,
    RemoveRelay(usize),
    SaveSettings,
}

/// Create a COSMIC application from the app model
impl cosmic::Application for AppModel {
    /// The async executor that will be used to run your application's commands.
    type Executor = cosmic::executor::Default;

    /// Data that your application receives to its init method.
    type Flags = ();

    /// Messages which the application and its widgets will emit.
    type Message = Message;

    /// Unique identifier in RDNN (reverse domain name notation) format.
    const APP_ID: &'static str = "com.github.pop-os.cosmic-app-template";

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    /// Initializes the application with any given flags and startup commands.
    fn init(
        core: cosmic::Core,
        _flags: Self::Flags,
    ) -> (Self, Task<cosmic::Action<Self::Message>>) {
        // Create a nav bar with two page items.
        let mut nav = nav_bar::Model::default();

        nav.insert()
            .text(fl!("compose"))
            .data::<Page>(Page::Compose)
            .icon(icon::from_name("document-edit-symbolic"))
            .activate();

        nav.insert()
            .text(fl!("settings"))
            .data::<Page>(Page::Settings)
            .icon(icon::from_name("preferences-system-symbolic"));

        // Load configuration
        let config = cosmic_config::Config::new(Self::APP_ID, Config::VERSION)
            .map(|context| match Config::get_entry(&context) {
                Ok(config) => config,
                Err((_errors, config)) => config,
            })
            .unwrap_or_default();

        // Construct the app model with the runtime's core.
        let mut app = AppModel {
            core,
            context_page: ContextPage::default(),
            nav,
            key_binds: HashMap::new(),
            temp_bluesky: config.bluesky.clone(),
            temp_mastodon: config.mastodon.clone(),
            temp_nostr: config.nostr.clone(),
            config,
            post_text: String::new(),
            posting_status: PostingStatus::Idle,
            new_relay: String::new(),
        };

        // Create a startup command that sets the window title.
        let command = app.update_title();

        (app, command)
    }

    /// Elements to pack at the start of the header bar.
    fn header_start(&self) -> Vec<Element<Self::Message>> {
        let menu_bar = menu::bar(vec![menu::Tree::with_children(
            menu::root(fl!("view")).apply(Element::from),
            menu::items(
                &self.key_binds,
                vec![menu::Item::Button(fl!("about"), None, MenuAction::About)],
            ),
        )]);

        vec![menu_bar.into()]
    }

    /// Enables the COSMIC application to create a nav bar with this model.
    fn nav_model(&self) -> Option<&nav_bar::Model> {
        Some(&self.nav)
    }

    /// Display a context drawer if the context page is requested.
    fn context_drawer(&self) -> Option<context_drawer::ContextDrawer<Self::Message>> {
        if !self.core.window.show_context {
            return None;
        }

        Some(match self.context_page {
            ContextPage::About => context_drawer::context_drawer(
                self.about(),
                Message::ToggleContextPage(ContextPage::About),
            )
            .title(fl!("about")),
        })
    }

    /// Describes the interface based on the current state of the application model.
    ///
    /// Application events will be processed through the view. Any messages emitted by
    /// events received by widgets will be passed to the update method.
    fn view(&self) -> Element<Self::Message> {
        let page = self.nav.data::<Page>(self.nav.active()).unwrap_or(&Page::Compose);
        
        match page {
            Page::Compose => self.compose_view(),
            Page::Settings => self.settings_view(),
        }
    }

    /// Register subscriptions for this application.
    ///
    /// Subscriptions are long-running async tasks running in the background which
    /// emit messages to the application through a channel. They are started at the
    /// beginning of the application, and persist through its lifetime.
    fn subscription(&self) -> Subscription<Self::Message> {
        struct MySubscription;

        Subscription::batch(vec![
            // Create a subscription which emits updates through a channel.
            Subscription::run_with_id(
                std::any::TypeId::of::<MySubscription>(),
                cosmic::iced::stream::channel(4, move |mut channel| async move {
                    _ = channel.send(Message::SubscriptionChannel).await;

                    futures_util::future::pending().await
                }),
            ),
            // Watch for application configuration changes.
            self.core()
                .watch_config::<Config>(Self::APP_ID)
                .map(|update| {
                    // for why in update.errors {
                    //     tracing::error!(?why, "app config error");
                    // }

                    Message::UpdateConfig(update.config)
                }),
        ])
    }

    /// Handles messages emitted by the application and its widgets.
    ///
    /// Tasks may be returned for asynchronous execution of code in the background
    /// on the application's async runtime.
    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::OpenRepositoryUrl => {
                _ = open::that_detached(REPOSITORY);
            }

            Message::SubscriptionChannel => {
                // For example purposes only.
            }

            Message::ToggleContextPage(context_page) => {
                if self.context_page == context_page {
                    self.core.window.show_context = !self.core.window.show_context;
                } else {
                    self.context_page = context_page;
                    self.core.window.show_context = true;
                }
            }

            Message::UpdateConfig(config) => {
                self.config = config.clone();
                self.temp_bluesky = config.bluesky;
                self.temp_mastodon = config.mastodon;
                self.temp_nostr = config.nostr;
            }

            Message::LaunchUrl(url) => match open::that_detached(&url) {
                Ok(()) => {}
                Err(err) => {
                    eprintln!("failed to open {url:?}: {err}");
                }
            },

            // Post composition
            Message::PostTextChanged(text) => {
                self.post_text = text;
            }

            Message::PostSubmit => {
                if self.post_text.trim().is_empty() {
                    return Task::none();
                }
                
                self.posting_status = PostingStatus::Posting;
                let config = self.config.clone();
                let text = self.post_text.clone();
                
                return Task::perform(
                    async move {
                        let mut results = Vec::new();
                        
                        if config.bluesky.enabled {
                            if let Err(e) = post_to_bluesky(&config.bluesky, &text).await {
                                results.push(e);
                            }
                        }
                        
                        if config.mastodon.enabled {
                            if let Err(e) = post_to_mastodon(&config.mastodon, &text).await {
                                results.push(e);
                            }
                        }
                        
                        if config.nostr.enabled {
                            if let Err(e) = post_to_nostr(&config.nostr, &text).await {
                                results.push(e);
                            }
                        }
                        
                        if results.is_empty() {
                            Ok(())
                        } else {
                            Err(results.into_iter().next().unwrap())
                        }
                    },
                    cosmic::Action::App(Message::PostResult)
                );
            }

            Message::PostResult(result) => {
                match result {
                    Ok(()) => {
                        self.posting_status = PostingStatus::Success;
                        self.post_text.clear();
                    }
                    Err(e) => {
                        self.posting_status = PostingStatus::Error(e.to_string());
                    }
                }
            }

            // Settings messages
            Message::BlueskyEnabledChanged(enabled) => {
                self.temp_bluesky.enabled = enabled;
            }
            Message::BlueskyHandleChanged(handle) => {
                self.temp_bluesky.handle = handle;
            }
            Message::BlueskyPasswordChanged(password) => {
                self.temp_bluesky.password = password;
            }
            Message::MastodonEnabledChanged(enabled) => {
                self.temp_mastodon.enabled = enabled;
            }
            Message::MastodonInstanceChanged(instance) => {
                self.temp_mastodon.instance_url = instance;
            }
            Message::MastodonTokenChanged(token) => {
                self.temp_mastodon.access_token = token;
            }
            Message::NostrEnabledChanged(enabled) => {
                self.temp_nostr.enabled = enabled;
            }
            Message::NostrPrivateKeyChanged(key) => {
                self.temp_nostr.private_key = key;
            }
            Message::NewRelayChanged(relay) => {
                self.new_relay = relay;
            }
            Message::AddRelay => {
                if !self.new_relay.trim().is_empty() {
                    self.temp_nostr.relays.push(self.new_relay.clone());
                    self.new_relay.clear();
                }
            }
            Message::RemoveRelay(index) => {
                if index < self.temp_nostr.relays.len() {
                    self.temp_nostr.relays.remove(index);
                }
            }
            Message::SaveSettings => {
                self.config.bluesky = self.temp_bluesky.clone();
                self.config.mastodon = self.temp_mastodon.clone();
                self.config.nostr = self.temp_nostr.clone();
                
                if let Ok(config_context) = cosmic_config::Config::new(Self::APP_ID, Config::VERSION) {
                    let _ = self.config.write_entry(&config_context);
                }
            }
        }
        Task::none()
    }

    /// Called when a nav item is selected.
    fn on_nav_select(&mut self, id: nav_bar::Id) -> Task<cosmic::Action<Self::Message>> {
        // Activate the page in the model.
        self.nav.activate(id);

        self.update_title()
    }
}

impl AppModel {
    /// The about page for this app.
    pub fn about(&self) -> Element<Message> {
        let cosmic_theme::Spacing { space_xxs, .. } = theme::active().cosmic().spacing;

        let icon = widget::svg(widget::svg::Handle::from_memory(APP_ICON));

        let title = widget::text::title3(fl!("app-title"));

        let hash = env!("VERGEN_GIT_SHA");
        let short_hash: String = hash.chars().take(7).collect();
        let date = env!("VERGEN_GIT_COMMIT_DATE");

        let link = widget::button::link(REPOSITORY)
            .on_press(Message::OpenRepositoryUrl)
            .padding(0);

        widget::column()
            .push(icon)
            .push(title)
            .push(link)
            .push(
                widget::button::link(fl!(
                    "git-description",
                    hash = short_hash.as_str(),
                    date = date
                ))
                .on_press(Message::LaunchUrl(format!("{REPOSITORY}/commits/{hash}")))
                .padding(0),
            )
            .align_x(Alignment::Center)
            .spacing(space_xxs)
            .into()
    }

    /// Updates the header and window titles.
    pub fn update_title(&mut self) -> Task<cosmic::Action<Message>> {
        let mut window_title = fl!("app-title");

        if let Some(page) = self.nav.text(self.nav.active()) {
            window_title.push_str(" — ");
            window_title.push_str(page);
        }

        if let Some(id) = self.core.main_window_id() {
            self.set_window_title(window_title, id)
        } else {
            Task::none()
        }
    }

    fn compose_view(&self) -> Element<Message> {
        let cosmic_theme::Spacing { space_xs, space_s, space_m, .. } = theme::active().cosmic().spacing;
        
        let text_input = text_input(&fl!("post-placeholder"), &self.post_text)
            .on_input(Message::PostTextChanged)
            .size(16);

        let char_count = self.post_text.len();
        let char_limit_text = widget::text(fl!("character-count", count = char_count as i32, limit = MAX_POST_LENGTH as i32))
            .size(12);

        let post_button = button(widget::text(fl!("post-button")))
            .on_press_maybe(if self.post_text.trim().is_empty() || char_count > MAX_POST_LENGTH {
                None
            } else {
                Some(Message::PostSubmit)
            })
            .style(theme::Button::Suggested);

        let status_text = match &self.posting_status {
            PostingStatus::Idle => None,
            PostingStatus::Posting => Some(widget::text(fl!("posting")).size(12)),
            PostingStatus::Success => Some(widget::text(fl!("post-success")).size(12)),
            PostingStatus::Error(err) => Some(widget::text(fl!("post-error", error = err.as_str())).size(12)),
        };

        let mut content = column()
            .push(text_input)
            .push(
                row()
                    .push(char_limit_text)
                    .push(widget::horizontal_space())
                    .push(post_button)
                    .align_y(Alignment::Center)
                    .spacing(space_s)
            )
            .spacing(space_s);

        if let Some(status) = status_text {
            content = content.push(status);
        }

        container(content)
            .padding(space_m)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn settings_view(&self) -> Element<Message> {
        let cosmic_theme::Spacing { space_xs, space_s, space_m, .. } = theme::active().cosmic().spacing;

        let bluesky_section = column()
            .push(widget::text::title4(fl!("bluesky-settings")))
            .push(
                checkbox(fl!("enable-account"), self.temp_bluesky.enabled)
                    .on_toggle(Message::BlueskyEnabledChanged)
            )
            .push(
                text_input(&fl!("handle"), &self.temp_bluesky.handle)
                    .on_input(Message::BlueskyHandleChanged)
            )
            .push(
                text_input(&fl!("password"), &self.temp_bluesky.password)
                    .on_input(Message::BlueskyPasswordChanged)
                    .password()
            )
            .spacing(space_xs);

        let mastodon_section = column()
            .push(widget::text::title4(fl!("mastodon-settings")))
            .push(
                checkbox(fl!("enable-account"), self.temp_mastodon.enabled)
                    .on_toggle(Message::MastodonEnabledChanged)
            )
            .push(
                text_input(&fl!("instance-url"), &self.temp_mastodon.instance_url)
                    .on_input(Message::MastodonInstanceChanged)
            )
            .push(
                text_input(&fl!("access-token"), &self.temp_mastodon.access_token)
                    .on_input(Message::MastodonTokenChanged)
                    .password()
            )
            .spacing(space_xs);

        let mut nostr_relays = column().spacing(space_xs);
        for (i, relay) in self.temp_nostr.relays.iter().enumerate() {
            nostr_relays = nostr_relays.push(
                row()
                    .push(widget::text(relay))
                    .push(widget::horizontal_space())
                    .push(
                        button(widget::text("Remove"))
                            .on_press(Message::RemoveRelay(i))
                            .style(theme::Button::Destructive)
                    )
                    .align_y(Alignment::Center)
                    .spacing(space_s)
            );
        }

        let add_relay_row = row()
            .push(
                text_input("wss://relay.example.com", &self.new_relay)
                    .on_input(Message::NewRelayChanged)
            )
            .push(
                button(widget::text(fl!("add-relay")))
                    .on_press(Message::AddRelay)
            )
            .spacing(space_s)
            .align_y(Alignment::Center);

        let nostr_section = column()
            .push(widget::text::title4(fl!("nostr-settings")))
            .push(
                checkbox(fl!("enable-account"), self.temp_nostr.enabled)
                    .on_toggle(Message::NostrEnabledChanged)
            )
            .push(
                text_input(&fl!("private-key"), &self.temp_nostr.private_key)
                    .on_input(Message::NostrPrivateKeyChanged)
                    .password()
            )
            .push(widget::text(fl!("relays")))
            .push(nostr_relays)
            .push(add_relay_row)
            .spacing(space_xs);

        let save_button = button(widget::text(fl!("save-settings")))
            .on_press(Message::SaveSettings)
            .style(theme::Button::Suggested);

        let content = column()
            .push(bluesky_section)
            .push(widget::rule::horizontal::default())
            .push(mastodon_section)
            .push(widget::rule::horizontal::default())
            .push(nostr_section)
            .push(save_button)
            .spacing(space_m);

        scrollable(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

/// The page to display in the application.
pub enum Page {
    Compose,
    Settings,
}

/// The context page to display in the context drawer.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum ContextPage {
    #[default]
    About,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuAction {
    About,
}

impl menu::action::MenuAction for MenuAction {
    type Message = Message;

    fn message(&self) -> Self::Message {
        match self {
            MenuAction::About => Message::ToggleContextPage(ContextPage::About),
        }
    }
}
