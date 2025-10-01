// SPDX-License-Identifier: GPL-3.0-only

use cosmic::app::{Core, Task};
use cosmic::iced::window::Id;
use cosmic::iced::Limits;
use cosmic::iced_winit::commands::popup::{destroy_popup, get_popup};
use cosmic::widget;
use cosmic::{Application, Element};

use crate::core::{Credentials, OpenVpnClient, Profile};

const AUTO_REFRESH_INTERVAL_SECS: u64 = 3;

/// Main application state
#[derive(Default)]
pub struct OpenVpn3Status {
    core: Core,
    popup: Option<Id>,
    client: OpenVpnClient,
    state: AppState,
}

/// Application state container
enum AppState {
    /// OpenVPN is not available on the system
    Unavailable,
    /// OpenVPN is available
    Available {
        profiles: Vec<Profile>,
        dialog: DialogState,
    },
}

impl Default for AppState {
    fn default() -> Self {
        Self::Unavailable
    }
}

/// Dialog state management
#[derive(Default)]
enum DialogState {
    #[default]
    Default,
    Import(ImportDialog),
    Connect(ConnectDialog),
}

struct ImportDialog {
    id: Id,
    file_path: String,
    custom_name: String,
}

impl ImportDialog {
    fn new(id: Id) -> Self {
        Self {
            id,
            file_path: String::new(),
            custom_name: String::new(),
        }
    }
}

struct ConnectDialog {
    id: Id,
    profile_name: String,
    username: String,
    password: String,
    totp: String,
    requires_totp: bool,
}

impl ConnectDialog {
    fn new(id: Id, profile_name: String, requires_totp: bool) -> Self {
        Self {
            id,
            profile_name,
            username: String::new(),
            password: String::new(),
            totp: String::new(),
            requires_totp,
        }
    }

    fn build_credentials(&self) -> Credentials {
        let mut creds = Credentials::new(&self.username, &self.password);
        if !self.totp.trim().is_empty() {
            creds = creds.with_totp(&self.totp);
        }
        creds
    }

    fn is_valid(&self) -> bool {
        !self.username.trim().is_empty()
            && !self.password.trim().is_empty()
            && (!self.requires_totp || !self.totp.trim().is_empty())
    }
}

/// Application messages
#[derive(Debug, Clone)]
pub enum Message {
    // Window management
    TogglePopup,
    PopupClosed(Id),

    // Profile operations
    RefreshProfiles,
    ProfilesLoaded(Result<Vec<Profile>, String>),
    DeleteProfile(String),
    ProfileDeleted(String, Result<(), String>),
    DisconnectSession(String),
    SessionDisconnected(String, Result<(), String>),

    // Import dialog
    OpenImportDialog,
    CloseImportDialog(Id),
    ImportFilePathChanged(String),
    ImportCustomNameChanged(String),
    SubmitImport,
    ConfigImported(Result<String, String>),

    // Connect dialog
    OpenConnectDialog { profile_name: String, requires_totp: bool },
    CloseConnectDialog(Id),
    ConnectUsernameChanged(String),
    ConnectPasswordChanged(String),
    ConnectTotpChanged(String),
    SubmitConnect,
    SessionStarted(Result<(), String>),

    // Auto-refresh
    AutoRefresh,
}

impl Application for OpenVpn3Status {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = "io.github.ryanhossain9797.OpenVPN3Status";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let client = OpenVpnClient::new();
        let app = Self {
            core,
            popup: None,
            client,
            state: AppState::default(),
        };

        // Initialize async
        let task = Task::perform(
            OpenVpnClient::is_available(),
            |should_load| {
                if should_load {
                    cosmic::Action::App(Message::RefreshProfiles)
                } else {
                    cosmic::Action::App(Message::ProfilesLoaded(Ok(Vec::new())))
                }
            },
        );

        (app, task)
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let icon = match &self.state {
            AppState::Available { profiles, .. } if profiles.iter().any(|p| p.is_active()) => {
                "logo_dark"
            }
            _ => "logo_dark_outline",
        };

        self.core
            .applet
            .icon_button(icon)
            .on_press(Message::TogglePopup)
            .into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        match &self.state {
            AppState::Unavailable => self.view_unavailable(),
            AppState::Available { dialog, .. } => match dialog {
                DialogState::Import(dialog) => self.view_import_dialog(dialog),
                DialogState::Connect(dialog) => self.view_connect_dialog(dialog),
                DialogState::Default => self.view_main_content(),
            },
        }
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::TogglePopup => self.handle_toggle_popup(),
            Message::PopupClosed(id) => self.handle_popup_closed(id),
            Message::RefreshProfiles => self.handle_refresh_profiles(),
            Message::ProfilesLoaded(result) => self.handle_profiles_loaded(result),
            Message::DeleteProfile(name) => self.handle_delete_profile(name),
            Message::ProfileDeleted(name, result) => self.handle_profile_deleted(name, result),
            Message::DisconnectSession(name) => self.handle_disconnect_session(name),
            Message::SessionDisconnected(name, result) => {
                self.handle_session_disconnected(name, result)
            }
            Message::OpenImportDialog => self.handle_open_import_dialog(),
            Message::CloseImportDialog(id) => self.handle_close_import_dialog(id),
            Message::ImportFilePathChanged(path) => {
                self.handle_import_file_path_changed(path);
                Task::none()
            }
            Message::ImportCustomNameChanged(name) => {
                self.handle_import_custom_name_changed(name);
                Task::none()
            }
            Message::SubmitImport => self.handle_submit_import(),
            Message::ConfigImported(result) => self.handle_config_imported(result),
            Message::OpenConnectDialog { profile_name, requires_totp } => {
                self.handle_open_connect_dialog(profile_name, requires_totp)
            }
            Message::CloseConnectDialog(id) => self.handle_close_connect_dialog(id),
            Message::ConnectUsernameChanged(username) => {
                self.handle_connect_username_changed(username);
                Task::none()
            }
            Message::ConnectPasswordChanged(password) => {
                self.handle_connect_password_changed(password);
                Task::none()
            }
            Message::ConnectTotpChanged(totp) => {
                self.handle_connect_totp_changed(totp);
                Task::none()
            }
            Message::SubmitConnect => self.handle_submit_connect(),
            Message::SessionStarted(result) => self.handle_session_started(result),
            Message::AutoRefresh => self.handle_auto_refresh(),
        }
    }

    fn style(&self) -> Option<cosmic::iced_runtime::Appearance> {
        Some(cosmic::applet::style())
    }
}

// View methods
impl OpenVpn3Status {
    fn view_unavailable(&self) -> Element<'_, Message> {
        let content = widget::column()
            .spacing(10)
            .padding(20)
            .push(widget::text("OpenVPN 3 not available").size(16));

        self.core.applet.popup_container(content).into()
    }

    fn view_main_content(&self) -> Element<'_, Message> {
        let AppState::Available { profiles, .. } = &self.state else {
            return self.view_unavailable();
        };

        let mut content = widget::column().spacing(10).padding(20);

        // Header
        content = content.push(widget::text("OpenVPN 3 Profiles").size(18));

        // Action buttons
        let refresh_button = widget::button::text("Refresh").on_press(Message::RefreshProfiles);
        let import_button = widget::button::text("Import Config").on_press(Message::OpenImportDialog);
        let button_row = widget::row()
            .push(refresh_button)
            .push(import_button)
            .spacing(10);
        content = content.push(button_row);
        content = content.push(widget::horizontal_space());

        // Profiles list
        if profiles.is_empty() {
            content = content.push(widget::text("No profiles found"));
        } else {
            for profile in profiles {
                content = content.push(self.view_profile_row(profile));
            }
        }

        self.core.applet.popup_container(content).into()
    }

    fn view_profile_row<'a>(&'a self, profile: &'a Profile) -> Element<'a, Message> {
        let name_text = widget::text(&profile.name).width(cosmic::iced::Length::Fill);

        let row = if profile.is_active() {
            let disconnect_btn = widget::button::text("Disconnect")
                .on_press(Message::DisconnectSession(profile.name.clone()));
            widget::row()
                .push(name_text)
                .push(disconnect_btn)
                .spacing(10)
        } else {
            let connect_btn = widget::button::text("Connect").on_press(
                Message::OpenConnectDialog {
                    profile_name: profile.name.clone(),
                    requires_totp: false, // Will be determined when dialog opens
                }
            );
            let delete_btn =
                widget::button::text("Delete").on_press(Message::DeleteProfile(profile.name.clone()));
            widget::row()
                .push(name_text)
                .push(connect_btn)
                .push(delete_btn)
                .spacing(10)
        };

        row.into()
    }

    fn view_import_dialog<'a>(&'a self, dialog: &'a ImportDialog) -> Element<'a, Message> {
        let file_path_input = widget::text_input("File path", &dialog.file_path)
            .on_input(Message::ImportFilePathChanged);

        let custom_name_input = widget::text_input("Custom name (optional)", &dialog.custom_name)
            .on_input(Message::ImportCustomNameChanged);

        let submit_button = widget::button::text("Import").on_press(Message::SubmitImport);
        let cancel_button =
            widget::button::text("Cancel").on_press(Message::CloseImportDialog(dialog.id));

        let button_row = widget::row()
            .push(submit_button)
            .push(cancel_button)
            .spacing(10);

        let dialog_content = widget::column()
            .push(widget::text("Import OpenVPN Config"))
            .push(file_path_input)
            .push(custom_name_input)
            .push(button_row)
            .spacing(10)
            .padding(20);

        let container = widget::container(dialog_content).width(400).height(200);

        self.core.applet.popup_container(container).into()
    }

    fn view_connect_dialog<'a>(&'a self, dialog: &'a ConnectDialog) -> Element<'a, Message> {
        let mut content = widget::column()
            .push(widget::text(format!("Connect to {}", dialog.profile_name)))
            .push(
                widget::text_input("Username", &dialog.username)
                    .on_input(Message::ConnectUsernameChanged),
            )
            .push(
                widget::text_input("Password", &dialog.password)
                    .password()
                    .on_input(Message::ConnectPasswordChanged),
            );

        if dialog.requires_totp {
            content = content.push(
                widget::text_input("TOTP Code (required)", &dialog.totp)
                    .on_input(Message::ConnectTotpChanged),
            );
        }

        let connect_button = widget::button::text("Connect").on_press(Message::SubmitConnect);
        let cancel_button =
            widget::button::text("Cancel").on_press(Message::CloseConnectDialog(dialog.id));

        let button_row = widget::row()
            .push(connect_button)
            .push(cancel_button)
            .spacing(10);

        content = content.push(button_row).spacing(10).padding(20);

        let height = if dialog.requires_totp { 280 } else { 250 };
        let container = widget::container(content).width(400).height(height);

        self.core.applet.popup_container(container).into()
    }
}

// Message handlers
impl OpenVpn3Status {
    /// Helper to get mutable reference to dialog if available
    fn dialog_mut(&mut self) -> Option<&mut DialogState> {
        match &mut self.state {
            AppState::Available { dialog, .. } => Some(dialog),
            AppState::Unavailable => None,
        }
    }

    /// Helper to get reference to dialog if available
    fn dialog(&self) -> Option<&DialogState> {
        match &self.state {
            AppState::Available { dialog, .. } => Some(dialog),
            AppState::Unavailable => None,
        }
    }

    fn handle_toggle_popup(&mut self) -> Task<Message> {
        if let Some(p) = self.popup.take() {
            destroy_popup(p)
        } else {
            // Refresh when opening
            let refresh_task = match &self.state {
                AppState::Available { .. } => self.handle_refresh_profiles(),
                AppState::Unavailable => Task::none(),
            };

            let new_id = Id::unique();
            self.popup.replace(new_id);
            let mut popup_settings = self.core.applet.get_popup_settings(
                self.core.main_window_id().unwrap(),
                new_id,
                None,
                None,
                None,
            );
            popup_settings.positioner.size_limits = Limits::NONE
                .max_width(372.0)
                .min_width(300.0)
                .min_height(200.0)
                .max_height(1080.0);

            Task::batch(vec![get_popup(popup_settings), refresh_task])
        }
    }

    fn handle_popup_closed(&mut self, id: Id) -> Task<Message> {
        if self.popup.as_ref() == Some(&id) {
            self.popup = None;
        }
        Task::none()
    }

    fn handle_refresh_profiles(&mut self) -> Task<Message> {
        let client = self.client.clone();
        Task::perform(
            async move {
                client
                    .get_profiles()
                    .await
                    .map_err(|e| e.to_string())
            },
            |result| cosmic::Action::App(Message::ProfilesLoaded(result)),
        )
    }

    fn handle_profiles_loaded(&mut self, result: Result<Vec<Profile>, String>) -> Task<Message> {
        match result {
            Ok(profiles) => {
                let has_connecting = profiles.iter().any(|p| p.is_connecting());

                self.state = AppState::Available {
                    profiles,
                    dialog: DialogState::Default,
                };

                if has_connecting {
                    Task::perform(
                        async {
                            tokio::time::sleep(tokio::time::Duration::from_secs(
                                AUTO_REFRESH_INTERVAL_SECS,
                            ))
                            .await;
                        },
                        |_| cosmic::Action::App(Message::AutoRefresh),
                    )
                } else {
                    Task::none()
                }
            }
            Err(e) => {
                eprintln!("Failed to load profiles: {}", e);
                Task::none()
            }
        }
    }

    fn handle_delete_profile(&mut self, name: String) -> Task<Message> {
        // Don't allow if dialog is open
        if !matches!(self.dialog(), Some(DialogState::Default)) {
            eprintln!("Cannot delete profile while dialog is open");
            return Task::none();
        }

        let client = self.client.clone();
        Task::perform(
            async move {
                let result = client.delete_profile(&name).await.map_err(|e| e.to_string());
                (name, result)
            },
            |(name, result)| cosmic::Action::App(Message::ProfileDeleted(name, result)),
        )
    }

    fn handle_profile_deleted(&mut self, name: String, result: Result<(), String>) -> Task<Message> {
        match result {
            Ok(_) => {
                if let AppState::Available { profiles, .. } = &mut self.state {
                    profiles.retain(|p| p.name != name);
                }
                Task::none()
            }
            Err(e) => {
                eprintln!("Failed to delete profile '{}': {}", name, e);
                Task::none()
            }
        }
    }

    fn handle_disconnect_session(&mut self, name: String) -> Task<Message> {
        // Don't allow if dialog is open
        if !matches!(self.dialog(), Some(DialogState::Default)) {
            eprintln!("Cannot disconnect session while dialog is open");
            return Task::none();
        }

        let client = self.client.clone();
        Task::perform(
            async move {
                let result = client.disconnect_profile(&name).await.map_err(|e| e.to_string());
                (name, result)
            },
            |(name, result)| cosmic::Action::App(Message::SessionDisconnected(name, result)),
        )
    }

    fn handle_session_disconnected(
        &mut self,
        name: String,
        result: Result<(), String>,
    ) -> Task<Message> {
        match result {
            Ok(_) => {
                eprintln!("Successfully disconnected session for '{}'", name);
                self.handle_refresh_profiles()
            }
            Err(e) => {
                eprintln!("Failed to disconnect session for '{}': {}", name, e);
                Task::none()
            }
        }
    }

    fn handle_open_import_dialog(&mut self) -> Task<Message> {
        // Don't open if another dialog is open or OpenVPN unavailable
        let Some(dialog) = self.dialog_mut() else {
            return Task::none();
        };

        if !matches!(dialog, DialogState::Default) {
            return Task::none();
        }

        let id = Id::unique();
        *dialog = DialogState::Import(ImportDialog::new(id));
        Task::none()
    }

    fn handle_close_import_dialog(&mut self, id: Id) -> Task<Message> {
        if let Some(DialogState::Import(dialog)) = self.dialog() {
            if dialog.id == id {
                if let Some(d) = self.dialog_mut() {
                    *d = DialogState::Default;
                }
            }
        }
        Task::none()
    }

    fn handle_import_file_path_changed(&mut self, path: String) {
        if let Some(DialogState::Import(dialog)) = self.dialog_mut() {
            dialog.file_path = path;
        }
    }

    fn handle_import_custom_name_changed(&mut self, name: String) {
        if let Some(DialogState::Import(dialog)) = self.dialog_mut() {
            dialog.custom_name = name;
        }
    }

    fn handle_submit_import(&mut self) -> Task<Message> {
        let (file_path, custom_name) = match self.dialog() {
            Some(DialogState::Import(dialog)) => {
                if dialog.file_path.trim().is_empty() {
                    return Task::none();
                }
                let name = if dialog.custom_name.trim().is_empty() {
                    None
                } else {
                    Some(dialog.custom_name.trim().to_string())
                };
                (dialog.file_path.clone(), name)
            }
            _ => return Task::none(),
        };

        if let Some(dialog) = self.dialog_mut() {
            *dialog = DialogState::Default;
        }

        let client = self.client.clone();
        Task::perform(
            async move {
                client
                    .import_config(&file_path, custom_name.as_deref())
                    .await
                    .map_err(|e| e.to_string())
            },
            |result| cosmic::Action::App(Message::ConfigImported(result)),
        )
    }

    fn handle_config_imported(&mut self, result: Result<String, String>) -> Task<Message> {
        match result {
            Ok(name) => {
                eprintln!("Successfully imported profile '{}'", name);
                self.handle_refresh_profiles()
            }
            Err(e) => {
                eprintln!("Failed to import config: {}", e);
                Task::none()
            }
        }
    }

    fn handle_open_connect_dialog(&mut self, profile_name: String, requires_totp: bool) -> Task<Message> {
        // Don't open if another dialog is open or OpenVPN unavailable
        let Some(dialog) = self.dialog_mut() else {
            return Task::none();
        };

        if !matches!(dialog, DialogState::Default) {
            return Task::none();
        }

        // If requires_totp is false, we need to check; otherwise just open the dialog
        if !requires_totp {
            let client = self.client.clone();
            let name = profile_name.clone();

            return Task::perform(
                async move {
                    let requires_totp = client.check_totp_required(&name).await.unwrap_or(false);
                    (name, requires_totp)
                },
                |(name, requires_totp)| cosmic::Action::App(Message::OpenConnectDialog {
                    profile_name: name,
                    requires_totp,
                }),
            );
        }

        // Open the dialog directly
        let id = Id::unique();
        *dialog = DialogState::Connect(ConnectDialog::new(id, profile_name, requires_totp));
        Task::none()
    }

    fn handle_close_connect_dialog(&mut self, id: Id) -> Task<Message> {
        if let Some(DialogState::Connect(dialog)) = self.dialog() {
            if dialog.id == id {
                if let Some(d) = self.dialog_mut() {
                    *d = DialogState::Default;
                }
            }
        }
        Task::none()
    }

    fn handle_connect_username_changed(&mut self, username: String) {
        if let Some(DialogState::Connect(dialog)) = self.dialog_mut() {
            dialog.username = username;
        }
    }

    fn handle_connect_password_changed(&mut self, password: String) {
        if let Some(DialogState::Connect(dialog)) = self.dialog_mut() {
            dialog.password = password;
        }
    }

    fn handle_connect_totp_changed(&mut self, totp: String) {
        if let Some(DialogState::Connect(dialog)) = self.dialog_mut() {
            dialog.totp = totp;
        }
    }

    fn handle_submit_connect(&mut self) -> Task<Message> {
        let (profile_name, credentials) = match self.dialog() {
            Some(DialogState::Connect(dialog)) => {
                if !dialog.is_valid() {
                    eprintln!("Please fill in all required fields");
                    return Task::none();
                }
                (dialog.profile_name.clone(), dialog.build_credentials())
            }
            _ => return Task::none(),
        };

        if let Some(dialog) = self.dialog_mut() {
            *dialog = DialogState::Default;
        }

        let client = self.client.clone();
        Task::perform(
            async move {
                client
                    .start_session(&profile_name, &credentials)
                    .await
                    .map_err(|e| e.to_string())
            },
            |result| cosmic::Action::App(Message::SessionStarted(result)),
        )
    }

    fn handle_session_started(&mut self, result: Result<(), String>) -> Task<Message> {
        match result {
            Ok(_) => {
                eprintln!("Successfully started session");
                self.handle_refresh_profiles()
            }
            Err(e) => {
                eprintln!("Failed to start session: {}", e);
                Task::none()
            }
        }
    }

    fn handle_auto_refresh(&mut self) -> Task<Message> {
        self.handle_refresh_profiles()
    }
}
