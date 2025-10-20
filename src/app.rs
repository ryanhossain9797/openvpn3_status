// SPDX-License-Identifier: GPL-3.0-only

use cosmic::app::{Core, Task};
use cosmic::iced::window::Id;
use cosmic::iced::Limits;
use cosmic::iced_winit::commands::popup::{destroy_popup, get_popup};
use cosmic::widget;
use cosmic::{Application, Element};

use crate::core::types::ConnectionStatus;
use crate::core::{CredentialInput, DynamicCredentials, OpenVpnClient, Profile};
use std::sync::Arc;

const AUTO_REFRESH_INTERVAL_SECS: u64 = 3;

/// Main application state
pub struct OpenVpn3Status {
    core: Core,
    popup: Option<Id>,
    client: Option<OpenVpnClient>,
    state: AppState,
}

impl Default for OpenVpn3Status {
    fn default() -> Self {
        Self {
            core: Core::default(),
            popup: None,
            client: None,
            state: AppState::default(),
        }
    }
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
    DynamicConnect(DynamicConnectDialog),
    WaitingForCredentialRequirements {
        profile_name: String,
        session_path: String,
    },
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

/// Dynamic credential dialog that shows fields based on server requirements
struct DynamicConnectDialog {
    id: Id,
    profile_name: String,
    session_path: String,
    required_inputs: Vec<CredentialInput>,
    input_values: std::collections::HashMap<u32, String>,
}

impl DynamicConnectDialog {
    fn new(
        id: Id,
        profile_name: String,
        session_path: String,
        required_inputs: Vec<CredentialInput>,
    ) -> Self {
        Self {
            id,
            profile_name,
            session_path,
            required_inputs,
            input_values: std::collections::HashMap::new(),
        }
    }

    fn is_valid(&self) -> bool {
        // All required inputs must have non-empty values
        self.required_inputs.iter().all(|input| {
            self.input_values
                .get(&input.unique_id())
                .map(|v| !v.trim().is_empty())
                .unwrap_or(false)
        })
    }

    fn build_credentials(&self) -> DynamicCredentials {
        let mut creds = DynamicCredentials::new();
        for (id, value) in &self.input_values {
            creds.add(*id, value.clone());
        }
        creds
    }
}

/// Application messages
#[derive(Debug, Clone)]
pub enum Message {
    // Window management
    TogglePopup,
    PopupClosed(Id),

    // Client initialization
    ClientInitialized(Option<OpenVpnClient>),

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

    // Dynamic credential flow (event-driven like openvpn3-indicator)
    StartTunnelCreation(String),                   // profile_name
    TunnelCreated(String, Result<String, String>), // profile_name, session_path
    CheckCredentialRequirements(String, String),   // profile_name, session_path
    CredentialRequirementsFetched(String, String, Result<Vec<CredentialInput>, String>), // profile_name, session_path, required_inputs
    DynamicInputChanged(u32, String), // input_id, value
    SubmitDynamicCredentials,
    DynamicCredentialsProvided(Result<(), String>),
    CancelDynamicConnect(String), // session_path - cancel and disconnect session
    SessionCancelled(Result<(), String>),

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
        let app = Self {
            core,
            popup: None,
            client: None,
            state: AppState::default(),
        };

        // Initialize async
        let task = Task::perform(
            async {
                let is_available = OpenVpnClient::is_available().await;
                if is_available {
                    match OpenVpnClient::new().await {
                        Ok(client) => Some(client),
                        Err(_) => None,
                    }
                } else {
                    None
                }
            },
            |client| cosmic::Action::App(Message::ClientInitialized(client)),
        );

        (app, task)
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let icon = match &self.state {
            AppState::Available { profiles, .. }
                if profiles.iter().any(|p| {
                    matches!(
                        p.status,
                        ConnectionStatus::Connected | ConnectionStatus::Connecting
                    )
                }) =>
            {
                "openvpn3_status_cosmic_logo_filled"
            }
            _ => "openvpn3_status_cosmic_logo_outline",
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
                DialogState::DynamicConnect(dialog) => self.view_dynamic_connect_dialog(dialog),
                DialogState::WaitingForCredentialRequirements { profile_name, .. } => {
                    self.view_waiting_dialog(profile_name)
                }
                DialogState::Default => self.view_main_content(),
            },
        }
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::TogglePopup => self.handle_toggle_popup(),
            Message::PopupClosed(id) => self.handle_popup_closed(id),
            Message::ClientInitialized(client) => self.handle_client_initialized(client),
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

            // Dynamic credential flow handlers
            Message::StartTunnelCreation(profile_name) => {
                self.handle_start_tunnel_creation(profile_name)
            }
            Message::TunnelCreated(profile_name, result) => {
                self.handle_tunnel_created(profile_name, result)
            }
            Message::CheckCredentialRequirements(profile_name, session_path) => {
                self.handle_check_credential_requirements(profile_name, session_path)
            }
            Message::CredentialRequirementsFetched(profile_name, session_path, result) => {
                self.handle_credential_requirements_fetched(profile_name, session_path, result)
            }
            Message::DynamicInputChanged(id, value) => {
                self.handle_dynamic_input_changed(id, value);
                Task::none()
            }
            Message::SubmitDynamicCredentials => self.handle_submit_dynamic_credentials(),
            Message::DynamicCredentialsProvided(result) => {
                self.handle_dynamic_credentials_provided(result)
            }
            Message::CancelDynamicConnect(session_path) => {
                self.handle_cancel_dynamic_connect(session_path)
            }
            Message::SessionCancelled(result) => self.handle_session_cancelled(result),

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
            .push(widget::text("Connecting to OpenVPN 3...").size(16))
            .push(widget::text("If this persists, check if OpenVPN 3 is installed").size(12));

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
        let refresh_button = widget::button::standard("Refresh").on_press(Message::RefreshProfiles);
        let import_button =
            widget::button::standard("Import Config").on_press(Message::OpenImportDialog);
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

        let row = match profile.status {
            ConnectionStatus::Connected | ConnectionStatus::Connecting => {
                let disconnect_btn = widget::button::destructive("Disconnect")
                    .on_press(Message::DisconnectSession(profile.name.clone()));
                widget::row()
                    .push(name_text)
                    .push(disconnect_btn)
                    .spacing(10)
            }
            ConnectionStatus::Disconnected | ConnectionStatus::Failed => {
                // Use the new dynamic credential flow that queries server requirements
                let connect_btn = widget::button::suggested("Connect")
                    .on_press(Message::StartTunnelCreation(profile.name.clone()));
                let delete_btn = widget::button::destructive("Delete")
                    .on_press(Message::DeleteProfile(profile.name.clone()));
                widget::row()
                    .push(name_text)
                    .push(connect_btn)
                    .push(delete_btn)
                    .spacing(10)
            }
        };

        row.into()
    }

    fn view_import_dialog<'a>(&'a self, dialog: &'a ImportDialog) -> Element<'a, Message> {
        let file_path_input = widget::text_input("File path", &dialog.file_path)
            .on_input(Message::ImportFilePathChanged);

        let custom_name_input = widget::text_input("Custom name (optional)", &dialog.custom_name)
            .on_input(Message::ImportCustomNameChanged);

        let submit_button = widget::button::suggested("Import").on_press(Message::SubmitImport);
        let cancel_button =
            widget::button::standard("Cancel").on_press(Message::CloseImportDialog(dialog.id));

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
            // When opening, check state and handle accordingly
            let refresh_task = match &self.state {
                AppState::Available { .. } => {
                    // Refresh profiles if already available
                    self.handle_refresh_profiles()
                }
                AppState::Unavailable => {
                    // Auto-retry initialization if service might be activatable
                    eprintln!("OpenVPN not available, attempting auto-retry...");
                    Self::retry_initialization()
                }
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

    fn handle_client_initialized(&mut self, client: Option<OpenVpnClient>) -> Task<Message> {
        self.client = client;
        if self.client.is_some() {
            self.handle_refresh_profiles()
        } else {
            Task::none()
        }
    }

    fn retry_initialization() -> Task<Message> {
        eprintln!("Retrying OpenVPN3 client initialization...");
        Task::perform(
            async {
                let is_available = OpenVpnClient::is_available().await;
                if is_available {
                    match OpenVpnClient::new().await {
                        Ok(client) => Some(client),
                        Err(_) => None,
                    }
                } else {
                    None
                }
            },
            |client| cosmic::Action::App(Message::ClientInitialized(client)),
        )
    }

    fn handle_refresh_profiles(&mut self) -> Task<Message> {
        let client = self.client.clone();
        Task::perform(
            async move {
                match client {
                    Some(client) => client.get_profiles().await.map_err(|e| e.to_string()),
                    None => Ok(Vec::new()),
                }
            },
            |result| cosmic::Action::App(Message::ProfilesLoaded(result)),
        )
    }

    fn handle_profiles_loaded(&mut self, result: Result<Vec<Profile>, String>) -> Task<Message> {
        match result {
            Ok(profiles) => {
                let has_connecting = profiles
                    .iter()
                    .any(|p| matches!(p.status, ConnectionStatus::Connecting));

                // Preserve existing dialog state when updating profiles
                match &mut self.state {
                    AppState::Available {
                        profiles: old_profiles,
                        ..
                    } => {
                        *old_profiles = profiles;
                    }
                    _ => {
                        self.state = AppState::Available {
                            profiles,
                            dialog: DialogState::Default,
                        };
                    }
                }

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
                let result = match client {
                    Some(client) => client
                        .delete_profile(&name)
                        .await
                        .map_err(|e| e.to_string()),
                    None => Err("OpenVPN client not available".to_string()),
                };
                (name, result)
            },
            |(name, result)| cosmic::Action::App(Message::ProfileDeleted(name, result)),
        )
    }

    fn handle_profile_deleted(
        &mut self,
        name: String,
        result: Result<(), String>,
    ) -> Task<Message> {
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
                let result = match client {
                    Some(client) => client
                        .disconnect_profile(&name)
                        .await
                        .map_err(|e| e.to_string()),
                    None => Err("OpenVPN client not available".to_string()),
                };
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
                match client {
                    Some(client) => client
                        .import_config(&file_path, custom_name.as_deref())
                        .await
                        .map_err(|e| e.to_string()),
                    None => Err("OpenVPN client not available".to_string()),
                }
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

    fn handle_auto_refresh(&mut self) -> Task<Message> {
        self.handle_refresh_profiles()
    }

    // Dynamic credential flow handlers
    fn handle_start_tunnel_creation(&mut self, profile_name: String) -> Task<Message> {
        let client = self.client.clone();
        let profile_name_arc = Arc::new(profile_name);
        let profile_name_for_task = Arc::clone(&profile_name_arc);
        let profile_name_for_msg = Arc::clone(&profile_name_arc);
        Task::perform(
            async move {
                match client {
                    Some(client) => client
                        .create_tunnel(&profile_name_for_task)
                        .await
                        .map_err(|e| e.to_string()),
                    None => Err("OpenVPN client not available".to_string()),
                }
            },
            move |result| {
                cosmic::Action::App(Message::TunnelCreated(
                    (*profile_name_for_msg).clone(),
                    result,
                ))
            },
        )
    }

    fn handle_tunnel_created(
        &mut self,
        profile_name: String,
        result: Result<String, String>,
    ) -> Task<Message> {
        match result {
            Ok(session_path) => {
                // Set waiting state
                if let Some(dialog) = self.dialog_mut() {
                    *dialog = DialogState::WaitingForCredentialRequirements {
                        profile_name: profile_name.clone(),
                        session_path: session_path.clone(),
                    };
                }

                // Use Arc for shared ownership
                let profile_name_arc = Arc::new(profile_name);
                let session_path_arc = Arc::new(session_path);
                let profile_name_for_msg = Arc::clone(&profile_name_arc);
                let session_path_for_msg = Arc::clone(&session_path_arc);

                // Poll for CFG_REQUIRE_USER state
                Task::perform(
                    async move {
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    },
                    move |_| {
                        cosmic::Action::App(Message::CheckCredentialRequirements(
                            (*profile_name_for_msg).clone(),
                            (*session_path_for_msg).clone(),
                        ))
                    },
                )
            }
            Err(e) => {
                eprintln!("Failed to create tunnel: {}", e);
                Task::none()
            }
        }
    }

    fn handle_check_credential_requirements(
        &mut self,
        profile_name: String,
        session_path: String,
    ) -> Task<Message> {
        let client = self.client.clone();
        let session_path_for_query = session_path.clone();
        let profile_name_arc = Arc::new(profile_name);
        let session_path_arc = Arc::new(session_path);
        let profile_name_for_msg = Arc::clone(&profile_name_arc);
        let session_path_for_msg = Arc::clone(&session_path_arc);
        Task::perform(
            async move {
                match client {
                    Some(client) => client
                        .query_required_inputs(&session_path_for_query)
                        .await
                        .map_err(|e| e.to_string()),
                    None => Err("OpenVPN client not available".to_string()),
                }
            },
            move |result| {
                cosmic::Action::App(Message::CredentialRequirementsFetched(
                    (*profile_name_for_msg).clone(),
                    (*session_path_for_msg).clone(),
                    result,
                ))
            },
        )
    }

    fn handle_credential_requirements_fetched(
        &mut self,
        profile_name: String,
        session_path: String,
        result: Result<Vec<CredentialInput>, String>,
    ) -> Task<Message> {
        match result {
            Ok(inputs) => {
                let id = Id::unique();
                if let Some(dialog) = self.dialog_mut() {
                    *dialog = DialogState::DynamicConnect(DynamicConnectDialog::new(
                        id,
                        profile_name,
                        session_path,
                        inputs,
                    ));
                }
                Task::none()
            }
            Err(e) => {
                eprintln!("Failed to fetch credential requirements: {}", e);
                if let Some(dialog) = self.dialog_mut() {
                    *dialog = DialogState::Default;
                }
                Task::none()
            }
        }
    }

    fn handle_dynamic_input_changed(&mut self, id: u32, value: String) {
        if let Some(DialogState::DynamicConnect(dialog)) = self.dialog_mut() {
            dialog.input_values.insert(id, value);
        }
    }

    fn handle_submit_dynamic_credentials(&mut self) -> Task<Message> {
        let (session_path, credentials) = match self.dialog() {
            Some(DialogState::DynamicConnect(dialog)) => {
                if !dialog.is_valid() {
                    eprintln!("Please fill in all required fields");
                    return Task::none();
                }
                (dialog.session_path.clone(), dialog.build_credentials())
            }
            _ => return Task::none(),
        };

        if let Some(dialog) = self.dialog_mut() {
            *dialog = DialogState::Default;
        }

        let client = self.client.clone();
        Task::perform(
            async move {
                match client {
                    Some(client) => {
                        client
                            .provide_dynamic_credentials(&session_path, &credentials)
                            .await?;
                        client.connect_session(&session_path).await?;
                        Ok(())
                    }
                    None => Err(crate::core::error::Error::InvalidInput(
                        "OpenVPN client not available".to_string(),
                    )),
                }
            },
            |result| {
                cosmic::Action::App(Message::DynamicCredentialsProvided(
                    result.map_err(|e| e.to_string()),
                ))
            },
        )
    }

    fn handle_dynamic_credentials_provided(&mut self, result: Result<(), String>) -> Task<Message> {
        match result {
            Ok(_) => {
                eprintln!("Successfully provided dynamic credentials and started session");
                self.handle_refresh_profiles()
            }
            Err(e) => {
                eprintln!("Failed to provide credentials: {}", e);
                Task::none()
            }
        }
    }

    fn handle_cancel_dynamic_connect(&mut self, session_path: String) -> Task<Message> {
        // Close the dialog immediately
        if let Some(dialog) = self.dialog_mut() {
            *dialog = DialogState::Default;
        }

        // Disconnect the session in the background
        let client = self.client.clone();
        Task::perform(
            async move {
                match client {
                    Some(client) => client
                        .disconnect_session(&session_path)
                        .await
                        .map_err(|e| e.to_string()),
                    None => Err("OpenVPN client not available".to_string()),
                }
            },
            |result| cosmic::Action::App(Message::SessionCancelled(result)),
        )
    }

    fn handle_session_cancelled(&mut self, result: Result<(), String>) -> Task<Message> {
        match result {
            Ok(_) => {
                eprintln!("Successfully cancelled and disconnected session");
                self.handle_refresh_profiles()
            }
            Err(e) => {
                eprintln!(
                    "Failed to disconnect session (may already be closed): {}",
                    e
                );
                // Refresh anyway to update the UI
                self.handle_refresh_profiles()
            }
        }
    }

    // View methods for dynamic dialog
    fn view_waiting_dialog<'a>(&'a self, profile_name: &'a str) -> Element<'a, Message> {
        // Get the session_path from the dialog state
        let session_path =
            if let Some(DialogState::WaitingForCredentialRequirements { session_path, .. }) =
                self.dialog()
            {
                session_path.clone()
            } else {
                String::new()
            };

        let cancel_button = widget::button::standard("Cancel")
            .on_press(Message::CancelDynamicConnect(session_path));

        let content = widget::column()
            .spacing(10)
            .padding(20)
            .push(widget::text(format!("Connecting to {}...", profile_name)))
            .push(widget::text("Waiting for server..."))
            .push(cancel_button);

        let container = widget::container(content).width(400).height(150);
        self.core.applet.popup_container(container).into()
    }

    fn view_dynamic_connect_dialog<'a>(
        &'a self,
        dialog: &'a DynamicConnectDialog,
    ) -> Element<'a, Message> {
        let mut content = widget::column()
            .push(widget::text(format!("Connect to {}", dialog.profile_name)))
            .spacing(10);

        // Build input fields dynamically based on server requirements
        for input in &dialog.required_inputs {
            let input_id = input.unique_id();
            let input_name = input.name.clone();
            let current_value = dialog
                .input_values
                .get(&input_id)
                .cloned()
                .unwrap_or_default();

            let text_input = if input.hidden {
                widget::text_input(input_name, current_value)
                    .password()
                    .on_input(move |v| Message::DynamicInputChanged(input_id, v))
            } else {
                widget::text_input(input_name, current_value)
                    .on_input(move |v| Message::DynamicInputChanged(input_id, v))
            };

            content = content.push(text_input);
        }

        // Buttons
        let connect_enabled = dialog.is_valid();
        let connect_button = if connect_enabled {
            widget::button::suggested("Connect").on_press(Message::SubmitDynamicCredentials)
        } else {
            widget::button::suggested("Connect")
        };

        let cancel_button = widget::button::standard("Cancel")
            .on_press(Message::CancelDynamicConnect(dialog.session_path.clone()));

        let button_row = widget::row()
            .push(connect_button)
            .push(cancel_button)
            .spacing(10);

        content = content.push(button_row).padding(20);

        // Dynamic height based on number of fields
        let height = 150.0 + (dialog.required_inputs.len() as f32 * 50.0);
        let container = widget::container(content).width(400.0).height(height);

        self.core.applet.popup_container(container).into()
    }
}
