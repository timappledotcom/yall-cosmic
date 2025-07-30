// SPDX-License-Identifier: MPL-2.0

/// Messages emitted by the application and its widgets.
#[derive(Debug, Clone)]
pub enum Message {
    MicroBlogEnabledChanged(bool),
    MicroBlogTokenChanged(String),
    SwitchView(ViewMode),
    UpdateConfig(Box<Config>),
    // Post composition
    PostEditorAction(text_editor::Action),
    PostSubmit,
    PostResult(Result<(), PostError>),
    PostToBlueskyToggled(bool),
    PostToMastodonToggled(bool),
    PostToMicroBlogToggled(bool),
    PostToNostrToggled(bool),
    AttachImage, // Open file picker
    ImageSelected(Option<String>), // Some(path) or None to clear

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
    ToggleRelays,

}
// SPDX-License-Identifier: MPL-2.0

use crate::config::{Config, BlueskyConfig, MastodonConfig, NostrConfig};
use rfd::FileDialog;
use crate::crypto::CryptoManager;
use crate::social::{self, PostError};

use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::iced::{Alignment, Length, Subscription};
use cosmic::prelude::*;
use cosmic::widget::{self, text_input, text_editor, checkbox, column, row, container, scrollable, divider, button};
use cosmic::iced_core::text::Wrapping;
use cosmic::{cosmic_theme, theme};




const MAX_POST_LENGTH: usize = 500;
const BLUESKY_LIMIT: usize = 300;

#[derive(Debug, Clone, Default)]
pub enum ViewMode {
    #[default]
    Compose,
    Settings,
}

#[derive(Debug, Clone, Default)]
pub enum PostingStatus {
    #[default]
    Idle,
    Posting,
    Success,
    Error(String),
}



/// The applet model stores app-specific state used to describe its interface and
/// drive its logic.
pub struct AppModel {
    /// Application state which is managed by the COSMIC runtime.
    core: cosmic::Core,
    /// Current view mode
    view_mode: ViewMode,
    // Configuration data that persists between application runs.
    config: Config,
    // UI state
    post_editor_content: text_editor::Content,
    posting_status: PostingStatus,
    post_to_bluesky: bool,
    post_to_mastodon: bool,
    post_to_microblog: bool,
    post_to_nostr: bool,
    attached_image: Option<String>, // Path to selected image
    // Settings editing state
    temp_bluesky: BlueskyConfig,
    temp_mastodon: MastodonConfig,
    temp_nostr: NostrConfig,
    temp_microblog: crate::config::MicroBlogConfig,
    new_relay: String,
    show_relays: bool,
    crypto_manager: CryptoManager,

}

/// Create a COSMIC application with system tray from the app model
impl cosmic::Application for AppModel {
    /// The async executor that will be used to run your applet's commands.
    type Executor = cosmic::executor::Default;

    /// Data that your applet receives to its init method.
    type Flags = ();

    /// Messages which the applet and its widgets will emit.
    type Message = Message;

    /// Unique identifier in RDNN (reverse domain name notation) format.
    const APP_ID: &'static str = "com.github.pop-os.yall-cosmic-applet";

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    /// Initializes the applet with any given flags and startup commands.
    fn init(
        core: cosmic::Core,
        _flags: Self::Flags,
    ) -> (Self, Task<cosmic::Action<Self::Message>>) {
        // Initialize crypto manager
        let mut crypto_manager = CryptoManager::new();
        if let Err(e) = crypto_manager.init_with_machine_key() {
            eprintln!("Failed to initialize crypto manager: {}", e);
        }

        // Load configuration
        let mut config = cosmic_config::Config::new(Self::APP_ID, Config::VERSION)
            .map(|context| match Config::get_entry(&context) {
                Ok(config) => config,
                Err((_errors, config)) => config,
            })
            .unwrap_or_default();

        // Decrypt credentials
        if let Err(e) = config.decrypt_credentials(&crypto_manager) {
            eprintln!("Failed to decrypt credentials: {}", e);
        }



        // Initialize temp configs with decrypted values
        let mut temp_bluesky = config.bluesky.clone();
        let mut temp_mastodon = config.mastodon.clone();
        let mut temp_nostr = config.nostr.clone();
        let mut temp_microblog = config.microblog.clone();

        // Copy decrypted values to temp configs
        temp_bluesky.decrypted_password = config.bluesky.decrypted_password.clone();
        temp_mastodon.decrypted_access_token = config.mastodon.decrypted_access_token.clone();
        temp_microblog.decrypted_access_token = config.microblog.decrypted_access_token.clone();
        temp_nostr.decrypted_private_key = config.nostr.decrypted_private_key.clone();

        // Construct the applet model with the runtime's core.
        let app = AppModel {
            core,
            view_mode: ViewMode::Compose,
            temp_bluesky,
            temp_mastodon,
            temp_nostr,
            temp_microblog,
            config: config.clone(),
            post_editor_content: text_editor::Content::new(),
            posting_status: PostingStatus::Idle,
            new_relay: String::new(),
            show_relays: true,
            post_to_bluesky: config.bluesky.enabled,
            post_to_mastodon: config.mastodon.enabled,
            post_to_microblog: config.microblog.enabled,
            post_to_nostr: config.nostr.enabled,
            attached_image: None,
            crypto_manager,
        };

        (app, Task::none())
    }

    /// Main view for the application
    fn view(&self) -> Element<Self::Message> {
        self.main_view()
    }

    /// Register subscriptions for this application.
    fn subscription(&self) -> Subscription<Self::Message> {
        Subscription::batch(vec![
            // Watch for configuration changes.
            self.core()
                .watch_config::<Config>(Self::APP_ID)
                .map(|update| Message::UpdateConfig(Box::new(update.config))),
        ])
    }

    /// Handle messages emitted by the applet and its widgets.
    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {

            Message::SwitchView(view_mode) => {
                self.view_mode = view_mode;
                Task::none()
            }
            Message::UpdateConfig(config) => {
                let mut config = *config;
                // Decrypt credentials when config is reloaded
                if let Err(e) = config.decrypt_credentials(&self.crypto_manager) {
                    eprintln!("Failed to decrypt credentials in UpdateConfig: {}", e);
                }
                
                eprintln!("UpdateConfig debug - config.nostr after decrypt:");
                eprintln!("  enabled: {}", config.nostr.enabled);
                eprintln!("  decrypted_private_key: '{}'", config.nostr.decrypted_private_key);
                eprintln!("  decrypted_private_key.len(): {}", config.nostr.decrypted_private_key.len());
                
                self.config = config;
                Task::none()
            }

            Message::PostEditorAction(action) => {
                self.post_editor_content.perform(action);
                Task::none()
            }
            Message::AttachImage => {
                // Open native file picker dialog and set attached_image
                let picked = FileDialog::new()
                    .add_filter("Image", &["png", "jpg", "jpeg", "gif", "bmp", "webp"])
                    .pick_file();
                let path = picked.map(|p| p.to_string_lossy().to_string());
                Task::done(cosmic::Action::App(Message::ImageSelected(path)))
            }
            Message::ImageSelected(path) => {
                self.attached_image = path;
                Task::none()
            }
            Message::PostSubmit => {
                let text = self.post_editor_content.text().to_string();
                if text.trim().is_empty() || text.chars().count() > MAX_POST_LENGTH {
                    return Task::none();
                }

                self.posting_status = PostingStatus::Posting;
                // Debug: Check self.config before creating copy
                eprintln!("PostSubmit debug - self.config.nostr before copy:");
                eprintln!("  enabled: {}", self.config.nostr.enabled);
                eprintln!("  decrypted_private_key: '{}'", self.config.nostr.decrypted_private_key);
                eprintln!("  decrypted_private_key.len(): {}", self.config.nostr.decrypted_private_key.len());

                // Create config copy with decrypted values (clone doesn't work due to #[serde(skip)])
                let mut config = self.config.clone();
                config.bluesky.decrypted_password = self.config.bluesky.decrypted_password.clone();
                config.mastodon.decrypted_access_token = self.config.mastodon.decrypted_access_token.clone();
                config.microblog.decrypted_access_token = self.config.microblog.decrypted_access_token.clone();
                config.nostr.decrypted_private_key = self.config.nostr.decrypted_private_key.clone();

                // Debug: Check config after manual copy
                eprintln!("PostSubmit debug - config.nostr after manual copy:");
                eprintln!("  enabled: {}", config.nostr.enabled);
                eprintln!("  decrypted_private_key: '{}'", config.nostr.decrypted_private_key);
                eprintln!("  decrypted_private_key.len(): {}", config.nostr.decrypted_private_key.len());
                let post_to_bluesky = self.post_to_bluesky;
                let post_to_mastodon = self.post_to_mastodon;
                let post_to_microblog = self.post_to_microblog;
                let post_to_nostr = self.post_to_nostr;

                let attached_image = self.attached_image.clone();
                eprintln!("PostSubmit debug - attached_image: {:?}", attached_image);
                eprintln!("PostSubmit debug - post_to_bluesky: {}", post_to_bluesky);
                eprintln!("PostSubmit debug - post_to_mastodon: {}", post_to_mastodon);
                eprintln!("PostSubmit debug - post_to_microblog: {}", post_to_microblog);
                eprintln!("PostSubmit debug - post_to_nostr: {}", post_to_nostr);

                Task::perform(
                    {
                        async move {
                            let mut errors = Vec::new();
                            let image_path = attached_image.as_deref();
                            eprintln!("PostSubmit debug - image_path: {:?}", image_path);
                            if post_to_bluesky {
                                eprintln!("PostSubmit: Posting to Bluesky...");
                                if let Err(e) = social::post_to_bluesky(&config.bluesky, &text, image_path).await {
                                    eprintln!("PostSubmit: Bluesky error: {}", e);
                                    errors.push(format!("Bluesky: {}", e));
                                }
                            }
                            if post_to_mastodon {
                                eprintln!("PostSubmit: Posting to Mastodon...");
                                if let Err(e) = social::post_to_mastodon(&config.mastodon, &text, image_path).await {
                                    eprintln!("PostSubmit: Mastodon error: {}", e);
                                    errors.push(format!("Mastodon: {}", e));
                                }
                            }
                            if post_to_microblog {
                                eprintln!("PostSubmit: Posting to Micro.Blog...");
                                if let Err(e) = social::post_to_microblog(&config.microblog, &text, image_path).await {
                                    eprintln!("PostSubmit: Micro.Blog error: {}", e);
                                    errors.push(format!("Micro.Blog: {}", e));
                                }
                            }
                            if post_to_nostr {
                                eprintln!("Attempting to post to Nostr...");
                                if let Err(e) = social::post_to_nostr(&config.nostr, &text, image_path).await {
                                    eprintln!("Nostr post failed: {}", e);
                                    errors.push(format!("Nostr: {}", e));
                                } else {
                                    eprintln!("Nostr post succeeded!");
                                }
                            } else {
                                eprintln!("Nostr posting is disabled (post_to_nostr = false)");
                            }
                            if errors.is_empty() {
                                Ok(())
                            } else {
                                Err(PostError::Api(errors.join("; ")))
                            }
                        }
                    },
                    |result| cosmic::Action::App(Message::PostResult(result)),
                )
            }
            Message::PostResult(result) => {
                match result {
                    Ok(()) => {
                        self.posting_status = PostingStatus::Success;
                        self.post_editor_content = text_editor::Content::new();
                    }
                    Err(e) => {
                        self.posting_status = PostingStatus::Error(e.to_string());
                    }
                }
                Task::none()
            }
            Message::PostToBlueskyToggled(enabled) => {
                self.post_to_bluesky = enabled;
                Task::none()
            }
            Message::PostToMastodonToggled(enabled) => {
                self.post_to_mastodon = enabled;
                Task::none()
            }
            Message::PostToMicroBlogToggled(enabled) => {
                self.post_to_microblog = enabled;
                Task::none()
            }
            Message::PostToNostrToggled(enabled) => {
                self.post_to_nostr = enabled;
                Task::none()
            }
            Message::BlueskyEnabledChanged(enabled) => {
                self.temp_bluesky.enabled = enabled;
                Task::none()
            }
            Message::BlueskyHandleChanged(handle) => {
                self.temp_bluesky.handle = handle;
                Task::none()
            }
            Message::BlueskyPasswordChanged(password) => {
                self.temp_bluesky.decrypted_password = password;
                Task::none()
            }
            Message::MastodonEnabledChanged(enabled) => {
                self.temp_mastodon.enabled = enabled;
                Task::none()
            }
            Message::MastodonInstanceChanged(instance) => {
                self.temp_mastodon.instance_url = instance;
                Task::none()
            }
            Message::MastodonTokenChanged(token) => {
                self.temp_mastodon.decrypted_access_token = token;
                Task::none()
            }
            Message::MicroBlogEnabledChanged(enabled) => {
                self.temp_microblog.enabled = enabled;
                Task::none()
            }
            Message::MicroBlogTokenChanged(token) => {
                self.temp_microblog.decrypted_access_token = token;
                Task::none()
            }
            Message::NostrEnabledChanged(enabled) => {
                self.temp_nostr.enabled = enabled;
                Task::none()
            }
            Message::NostrPrivateKeyChanged(key) => {
                self.temp_nostr.decrypted_private_key = key;
                Task::none()
            }
            Message::NewRelayChanged(relay) => {
                self.new_relay = relay;
                Task::none()
            }
            Message::AddRelay => {
                if Self::validate_relay_url(&self.new_relay) && !self.temp_nostr.relays.contains(&self.new_relay) {
                    self.temp_nostr.relays.push(self.new_relay.clone());
                    self.new_relay.clear();
                }
                Task::none()
            }
            Message::RemoveRelay(index) => {
                if index < self.temp_nostr.relays.len() {
                    self.temp_nostr.relays.remove(index);
                }
                Task::none()
            }
            Message::SaveSettings => {
                // Debug: Check temp_nostr values before saving
                eprintln!("SaveSettings debug - temp_nostr:");
                eprintln!("  enabled: {}", self.temp_nostr.enabled);
                eprintln!("  decrypted_private_key: '{}'", self.temp_nostr.decrypted_private_key);
                eprintln!("  decrypted_private_key.len(): {}", self.temp_nostr.decrypted_private_key.len());

                // Update config with temp values
                self.config.bluesky = self.temp_bluesky.clone();
                self.config.mastodon = self.temp_mastodon.clone();
                self.config.microblog = self.temp_microblog.clone();
                self.config.nostr = self.temp_nostr.clone();

                // Debug: Check main config after copying
                eprintln!("SaveSettings debug - config.nostr after copy:");
                eprintln!("  enabled: {}", self.config.nostr.enabled);
                eprintln!("  decrypted_private_key: '{}'", self.config.nostr.decrypted_private_key);
                eprintln!("  decrypted_private_key.len(): {}", self.config.nostr.decrypted_private_key.len());
                
                // Encrypt credentials before saving
                if let Err(e) = self.config.encrypt_credentials(&self.crypto_manager) {
                    eprintln!("Failed to encrypt credentials: {}", e);
                    self.posting_status = PostingStatus::Error("Failed to save settings".to_string());
                    return Task::none();
                }
                
                // Update posting toggles based on new config
                self.post_to_bluesky = self.config.bluesky.enabled;
                self.post_to_mastodon = self.config.mastodon.enabled;
                self.post_to_microblog = self.config.microblog.enabled;
                self.post_to_nostr = self.config.nostr.enabled;

                if let Ok(config_context) = cosmic_config::Config::new(Self::APP_ID, Config::VERSION) {
                    if let Err(e) = self.config.write_entry(&config_context) {
                        eprintln!("Failed to save config: {}", e);
                        self.posting_status = PostingStatus::Error("Failed to save settings".to_string());
                    } else {
                        self.posting_status = PostingStatus::Success;
                    }
                } else {
                    self.posting_status = PostingStatus::Error("Failed to save settings".to_string());
                }
                
                // Decrypt again for runtime use
                if let Err(e) = self.config.decrypt_credentials(&self.crypto_manager) {
                    eprintln!("Failed to decrypt credentials after save: {}", e);
                } else {
                    eprintln!("SaveSettings debug - config.nostr after decrypt:");
                    eprintln!("  enabled: {}", self.config.nostr.enabled);
                    eprintln!("  decrypted_private_key: '{}'", self.config.nostr.decrypted_private_key);
                    eprintln!("  decrypted_private_key.len(): {}", self.config.nostr.decrypted_private_key.len());
                }
                
                Task::none()
            }
            Message::ToggleRelays => {
                self.show_relays = !self.show_relays;
                Task::none()
            }


        }
    }

}

impl AppModel {
    fn validate_url(url: &str) -> bool {
        url.starts_with("https://") && url.len() > 8
    }

    fn validate_handle(handle: &str) -> bool {
        !handle.is_empty() && handle.contains('.')
    }

    fn validate_private_key(key: &str) -> bool {
        key.len() == 64 && key.chars().all(|c| c.is_ascii_hexdigit())
    }

    fn validate_relay_url(url: &str) -> bool {
        url.starts_with("wss://") && url.len() > 6
    }
    fn compose_view(&self) -> Element<Message> {
        let cosmic_theme::Spacing { space_s, .. } = theme::active().cosmic().spacing;

        let text_editor_widget = container(
            text_editor(&self.post_editor_content)
                .placeholder("What's happening?")
                .on_action(Message::PostEditorAction)
                .height(200.0)
                .width(500.0)
                .wrapping(Wrapping::Word)
        )
        .padding(space_s);

        let char_count = self.post_editor_content.text().chars().count();
        let char_limit_text = widget::text(format!("{}/{}", char_count, MAX_POST_LENGTH))
            .size(12);

        // Show Bluesky warning if over 300 characters and Bluesky is enabled
        let bluesky_warning = if char_count > BLUESKY_LIMIT && self.post_to_bluesky && 
            self.temp_bluesky.enabled && !self.temp_bluesky.handle.is_empty() && !self.temp_bluesky.decrypted_password.is_empty() {
            Some(widget::text(format!("⚠️ Bluesky posts will be truncated to {} characters", BLUESKY_LIMIT))
                .size(11))
        } else {
            None
        };

        let post_button = if self.post_editor_content.text().trim().is_empty() || char_count > MAX_POST_LENGTH {
            widget::button::suggested("Post")
        } else {
            widget::button::suggested("Post")
                .on_press(Message::PostSubmit)
        };

        let status_text = match &self.posting_status {
            PostingStatus::Idle => None,
            PostingStatus::Posting => Some(widget::text("Posting...").size(12)),
            PostingStatus::Success => Some(widget::text("Posted successfully!").size(12)),
            PostingStatus::Error(err) => Some(widget::text(format!("Failed to post: {}", err)).size(12)),
        };

        let mut checkboxes = row().spacing(space_s);

        // Only show checkboxes for configured platforms
        if self.temp_mastodon.enabled && !self.temp_mastodon.instance_url.is_empty() && !self.temp_mastodon.decrypted_access_token.is_empty() {
            checkboxes = checkboxes.push(checkbox("Mastodon", self.post_to_mastodon).on_toggle(Message::PostToMastodonToggled));
        }
        if self.temp_bluesky.enabled && !self.temp_bluesky.handle.is_empty() && !self.temp_bluesky.decrypted_password.is_empty() {
            checkboxes = checkboxes.push(checkbox("Bluesky", self.post_to_bluesky).on_toggle(Message::PostToBlueskyToggled));
        }
        if self.temp_microblog.enabled && !self.temp_microblog.decrypted_access_token.is_empty() {
            checkboxes = checkboxes.push(checkbox("Micro.Blog", self.post_to_microblog).on_toggle(Message::PostToMicroBlogToggled));
        }
        if self.temp_nostr.enabled && !self.temp_nostr.decrypted_private_key.is_empty() && !self.temp_nostr.relays.is_empty() {
            checkboxes = checkboxes.push(checkbox("Nostr", self.post_to_nostr).on_toggle(Message::PostToNostrToggled));
        }

        // Image attachment section
        let mut image_section = row().spacing(space_s);
        
        let attach_button = widget::button::standard("📎 Attach Image")
            .on_press(Message::AttachImage);
        image_section = image_section.push(attach_button);
        
        if let Some(ref image_path) = self.attached_image {
            let filename = std::path::Path::new(image_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("image");
            image_section = image_section.push(widget::text(format!("📷 {}", filename)).size(12));
            
            let clear_button = widget::button::destructive("✕")
                .on_press(Message::ImageSelected(None));
            image_section = image_section.push(clear_button);
        }

        let mut content = column()
            .push(text_editor_widget)
            .push(image_section)
            .push(checkboxes)
            .spacing(space_s);

        if let Some(warning) = bluesky_warning {
            content = content.push(warning);
        }

        content = content.push(
            row()
                .push(char_limit_text)
                .push(widget::horizontal_space())
                .push(post_button)
                .align_y(Alignment::Center)
                .spacing(space_s)
        );

        if let Some(status) = status_text {
            content = content.push(status);
        }

        content.into()
    }

    fn settings_view(&self) -> Element<Message> {
        let cosmic_theme::Spacing { space_xs, space_s, space_m, .. } = theme::active().cosmic().spacing;
        let space_xs = (space_xs as f32 * 0.75) as u16;
        let space_s = (space_s as f32 * 0.75) as u16;
        let space_m = (space_m as f32 * 0.75) as u16;

        let mut bluesky_section = column()
            .push(widget::text::title4("Bluesky Settings"))
            .push(
                checkbox("Enable", self.temp_bluesky.enabled)
                    .on_toggle(Message::BlueskyEnabledChanged)
            )
            .push(
                text_input("Handle (e.g., user.bsky.social)", &self.temp_bluesky.handle)
                    .on_input(Message::BlueskyHandleChanged)
                    .width(Length::Fill)
            );

        if self.temp_bluesky.enabled && !self.temp_bluesky.handle.is_empty() && !Self::validate_handle(&self.temp_bluesky.handle) {
            bluesky_section = bluesky_section.push(widget::text("Invalid handle format").size(12));
        }

        bluesky_section = bluesky_section
            .push(
                text_input("App Password", &self.temp_bluesky.decrypted_password)
                    .on_input(Message::BlueskyPasswordChanged)
                    .password()
                    .width(Length::Fill)
            )
            .spacing(space_xs);

        let mut mastodon_section = column()
            .push(widget::text::title4("Mastodon Settings"))
            .push(
                checkbox("Enable", self.temp_mastodon.enabled)
                    .on_toggle(Message::MastodonEnabledChanged)
            )
            .push(
                text_input("Instance URL (e.g., https://mastodon.social)", &self.temp_mastodon.instance_url)
                    .on_input(Message::MastodonInstanceChanged)
                    .width(Length::Fill)
            );

        if self.temp_mastodon.enabled && !self.temp_mastodon.instance_url.is_empty() && !Self::validate_url(&self.temp_mastodon.instance_url) {
            mastodon_section = mastodon_section.push(widget::text("Invalid URL format").size(12));
        }

        mastodon_section = mastodon_section
            .push(
                text_input("Access Token", &self.temp_mastodon.decrypted_access_token)
                    .on_input(Message::MastodonTokenChanged)
                    .password()
                    .width(Length::Fill)
            )
            .spacing(space_xs);

        let microblog_section = column()
            .push(widget::text::title4("Micro.Blog Settings"))
            .push(
                checkbox("Enable", self.temp_microblog.enabled)
                    .on_toggle(Message::MicroBlogEnabledChanged)
            )
            .push(
                text_input("Access Token", &self.temp_microblog.decrypted_access_token)
                    .on_input(Message::MicroBlogTokenChanged)
                    .password()
                    .width(Length::Fill)
            )
            .spacing(space_xs);

        // Collapsible Nostr relays
        let relays_toggle = widget::button::standard(if self.show_relays { "Hide Relays" } else { "Show Relays" })
            .on_press(Message::ToggleRelays);

        let mut nostr_relays = column().spacing(space_xs);
        if self.show_relays {
            for (i, relay) in self.temp_nostr.relays.iter().enumerate() {
                nostr_relays = nostr_relays.push(
                    row()
                        .push(widget::text(relay))
                        .push(widget::horizontal_space())
                        .push(
                            widget::button::destructive("Remove")
                                .on_press(Message::RemoveRelay(i))
                        )
                        .align_y(Alignment::Center)
                        .spacing(space_s)
                );
            }
        }

        let add_relay_row = row()
            .push(
                text_input("wss://relay.example.com", &self.new_relay)
                    .on_input(Message::NewRelayChanged)
                    .width(Length::Fill)
            )
            .push(
                widget::button::standard("Add Relay")
                    .on_press(Message::AddRelay)
            )
            .spacing(space_s)
            .align_y(Alignment::Center);

        let mut nostr_section = column()
            .push(widget::text::title4("Nostr Settings"))
            .push(
                checkbox("Enable", self.temp_nostr.enabled)
                    .on_toggle(Message::NostrEnabledChanged)
            )
            .push(
                text_input("Private Key (64 hex characters)", &self.temp_nostr.decrypted_private_key)
                    .on_input(Message::NostrPrivateKeyChanged)
                    .password()
                    .width(Length::Fill)
            );

        if self.temp_nostr.enabled && !self.temp_nostr.decrypted_private_key.is_empty() && !Self::validate_private_key(&self.temp_nostr.decrypted_private_key) {
            nostr_section = nostr_section.push(widget::text("Invalid private key format (must be 64 hex characters)").size(12));
        }

        nostr_section = nostr_section
            .push(widget::text("Relays"))
            .push(relays_toggle)
            .push(nostr_relays)
            .push(add_relay_row)
            .spacing(space_xs);

        let save_button = widget::button::suggested("Save Settings")
            .on_press(Message::SaveSettings);

        let content = column()
            .push(bluesky_section)
            .push(divider::horizontal::default())
            .push(mastodon_section)
            .push(divider::horizontal::default())
            .push(microblog_section)
            .push(divider::horizontal::default())
            .push(nostr_section)
            .push(save_button)
            .spacing(space_m);

        // Add extra right padding inside the scrollable content to prevent scrollbar overlap
        scrollable(
            container(content)
                .padding([space_m, space_m * 2, space_m, space_m]) // top, right, bottom, left
                .width(Length::Fill)
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }



    fn main_view(&self) -> Element<Message> {
        let cosmic_theme::Spacing { space_s, space_m, .. } = theme::active().cosmic().spacing;

        let view_buttons = row()
            .push(
                button::standard("Compose")
                    .on_press_maybe(if matches!(self.view_mode, ViewMode::Compose) {
                        None
                    } else {
                        Some(Message::SwitchView(ViewMode::Compose))
                    })
            )
            .push(
                button::standard("Settings")
                    .on_press_maybe(if matches!(self.view_mode, ViewMode::Settings) {
                        None
                    } else {
                        Some(Message::SwitchView(ViewMode::Settings))
                    })
            )
            .spacing(space_s);

        let content = column()
            .push(view_buttons)
            .push(match self.view_mode {
                ViewMode::Compose => self.compose_view(),
                ViewMode::Settings => self.settings_view(),
            })
            .spacing(space_s);

        container(content)
            .padding(space_m)
            .width(Length::Fill)
            .height(Length::Fixed(530.0))
            .into()
    }




}


