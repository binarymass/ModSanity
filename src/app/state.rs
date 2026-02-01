//! Application state management

use crate::collections::Collection;
use crate::db::CategoryRecord;
use crate::games::Game;
use crate::mods::fomod::{ConditionEvaluator, FileInstruction, FomodInstaller, WizardState};
use crate::mods::InstalledMod;
use crate::plugins::PluginInfo;
use crate::profiles::Profile;
use std::path::PathBuf;

/// Current screen in the TUI
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Screen {
    #[default]
    Dashboard,
    Mods,
    Plugins,
    Profiles,
    Settings,
    ModDetails,
    FomodWizard,
    GameSelect,
    Collection,
    Browse,
    LoadOrder,
}

/// Application state for TUI
#[derive(Debug, Default)]
pub struct AppState {
    /// Currently active game
    pub active_game: Option<Game>,

    /// Current screen
    pub current_screen: Screen,

    /// Previous screen (for back navigation)
    pub previous_screen: Option<Screen>,

    /// Selected mod index in list
    pub selected_mod_index: usize,

    /// Selected plugin index
    pub selected_plugin_index: usize,

    /// Selected download index

    /// Selected profile index
    pub selected_profile_index: usize,

    /// Selected search result index

    /// Selected game index (for game selection)
    pub selected_game_index: usize,

    /// Selected setting index
    pub selected_setting_index: usize,

    /// Installed mods (cached for display)
    pub installed_mods: Vec<InstalledMod>,

    /// Plugins (cached for display)
    pub plugins: Vec<PluginInfo>,

    /// Profiles (cached for display)
    pub profiles: Vec<Profile>,

    /// Status message
    pub status_message: Option<String>,

    /// Show help panel
    pub show_help: bool,

    /// Show confirmation dialog
    pub show_confirm: Option<ConfirmDialog>,

    /// Show requirements dialog
    pub show_requirements: Option<RequirementsDialog>,

    /// Input mode (for text input)
    pub input_mode: InputMode,

    /// Current input buffer
    pub input_buffer: String,

    /// Should quit
    pub should_quit: bool,

    /// Is loading data
    pub is_loading: bool,

    /// Error message to display
    pub error_message: Option<String>,

    /// Installation progress (0-100)
    pub installation_progress: Option<InstallProgress>,

    /// Categorization progress
    pub categorization_progress: Option<CategorizationProgress>,

    /// FOMOD components for selection
    pub fomod_components: Vec<crate::mods::fomod::FomodComponent>,

    /// Selected FOMOD component indices
    pub selected_fomod_components: Vec<usize>,

    /// Current FOMOD component index in selection
    pub fomod_selection_index: usize,

    /// Archive path being installed (for FOMOD continuation)
    pub pending_install_archive: Option<String>,

    /// Available categories (cached for display)
    pub categories: Vec<CategoryRecord>,

    /// Selected category index in category list
    pub selected_category_index: usize,

    /// Active category filter (None = show all, Some(id) = filter by category)
    pub category_filter: Option<i64>,

    /// Search query for filtering mods by name
    pub mod_search_query: String,

    /// Search query for filtering plugins by name
    pub plugin_search_query: String,

    /// Currently loaded collection
    pub current_collection: Option<Collection>,

    /// Selected collection mod index
    pub selected_collection_mod_index: usize,

    /// Collection mod install status (mod_id -> is_installed)
    pub collection_mod_status: std::collections::HashMap<i64, bool>,

    /// Available mod updates (mod_id -> update info)
    pub available_updates: std::collections::HashMap<i64, crate::nexus::graphql::ModUpdateInfo>,

    /// Whether we're currently checking for updates
    pub checking_updates: bool,

    /// Browse/search results from Nexus Mods
    pub browse_results: Vec<crate::nexus::graphql::ModSearchResult>,

    /// Selected browse result index
    pub selected_browse_index: usize,

    /// Current browse search query
    pub browse_query: String,

    /// Current browse sort order
    pub browse_sort: crate::nexus::graphql::SortBy,

    /// Browse offset for paginated results
    pub browse_offset: i32,

    /// Browse page size for results
    pub browse_limit: i32,

    /// Total results for current browse query
    pub browse_total_count: i64,

    /// Whether a browse search is in progress
    pub browsing: bool,

    /// Whether we're showing default browse content (top mods) vs search results
    pub browse_showing_default: bool,

    /// Files available for the selected browse mod
    pub browse_mod_files: Vec<crate::nexus::graphql::ModFile>,

    /// Selected file index in file list
    pub selected_file_index: usize,

    /// Whether we're showing the file picker for a mod
    pub showing_file_picker: bool,

    /// Context for the mod being downloaded (mod_id, mod_name, game_domain)
    pub download_context: Option<DownloadContext>,

    /// Download progress (bytes downloaded, total bytes)
    pub download_progress: Option<DownloadProgress>,

    /// Whether the user is in reorder mode on the Load Order screen
    pub reorder_mode: bool,

    /// Selected index in the load order list
    pub load_order_index: usize,

    /// Whether the user is in reorder mode on the Plugins screen
    pub plugin_reorder_mode: bool,

    /// Whether the plugin list has unsaved changes
    pub plugin_dirty: bool,

    /// Working copy of mods for reordering (snapshot, not persisted until save)
    pub load_order_mods: Vec<InstalledMod>,

    /// Cached conflict data for the load order screen
    pub load_order_conflicts: Vec<crate::mods::ModConflict>,

    /// Whether the load order has unsaved changes
    pub load_order_dirty: bool,

    /// FOMOD wizard state (when showing full wizard UI)
    pub fomod_wizard_state: Option<FomodWizardState>,
}

/// Context for an active download
#[derive(Debug, Clone)]
pub struct DownloadContext {
    pub mod_id: i64,
    pub mod_name: String,
    pub game_domain: String,
    pub game_id: i64,
}

/// Download progress information
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub file_name: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
}

/// Installation progress information
#[derive(Debug, Clone)]
pub struct InstallProgress {
    /// Progress percentage for current mod (0-100)
    pub percent: u16,
    /// Current file being processed
    pub current_file: String,
    /// Total files to process in current mod
    pub total_files: usize,
    /// Files processed so far in current mod
    pub processed_files: usize,

    /// Bulk install: current mod name (if bulk installing)
    pub current_mod_name: Option<String>,
    /// Bulk install: current mod index (1-based)
    pub current_mod_index: Option<usize>,
    /// Bulk install: total mods to install
    pub total_mods: Option<usize>,
}

/// Categorization progress information
#[derive(Debug, Clone)]
pub struct CategorizationProgress {
    /// Current mod index (1-based)
    pub current_index: usize,
    /// Total mods to categorize
    pub total_mods: usize,
    /// Current mod name
    pub current_mod_name: String,
    /// Number categorized so far
    pub categorized_count: usize,
}

/// FOMOD wizard state
#[derive(Debug)]
pub struct FomodWizardState {
    /// The FOMOD installer being used
    pub installer: FomodInstaller,
    /// Wizard state with selections and evaluator
    pub wizard: WizardState,

    // UI state
    /// Current step index (0-based)
    pub current_step: usize,
    /// Current group index within step (0-based)
    pub current_group: usize,
    /// Selected option index within group (0-based)
    pub selected_option: usize,
    /// Validation errors for current selections
    pub validation_errors: Vec<String>,

    // Context
    /// Name of the mod being installed
    pub mod_name: String,
    /// Staging path where mod files are extracted
    pub staging_path: PathBuf,
    /// Preview of files to install (computed lazily)
    pub preview_files: Option<Vec<FileInstruction>>,
    /// If Some, this is a reconfiguration of existing mod with this ID
    pub existing_mod_id: Option<i64>,

    /// Current phase of the wizard
    pub phase: WizardPhase,
}

/// Wizard phases for UI flow
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardPhase {
    /// Show mod metadata and info
    Overview,
    /// Navigate through steps and make selections
    StepNavigation,
    /// Preview selections and files
    Summary,
    /// Final confirmation
    Confirm,
}

impl FomodWizardState {
    /// Create new wizard state
    pub fn new(
        installer: FomodInstaller,
        wizard: WizardState,
        mod_name: String,
        staging_path: PathBuf,
        existing_mod_id: Option<i64>,
    ) -> Self {
        Self {
            installer,
            wizard,
            current_step: 0,
            current_group: 0,
            selected_option: 0,
            validation_errors: Vec::new(),
            mod_name,
            staging_path,
            preview_files: None,
            existing_mod_id,
            phase: WizardPhase::Overview,
        }
    }

    /// Get the current install step
    pub fn current_install_step(&self) -> Option<&crate::mods::fomod::InstallStep> {
        self.installer.steps().get(self.current_step)
    }

    /// Get the current option group
    pub fn current_option_group(&self) -> Option<&crate::mods::fomod::OptionGroup> {
        self.current_install_step()
            .and_then(|step| step.groups.groups.get(self.current_group))
    }

    /// Check if we can proceed to next step
    pub fn can_proceed(&self) -> bool {
        self.validation_errors.is_empty()
    }

    /// Advance to next step
    pub fn next_step(&mut self) {
        if self.current_step + 1 < self.installer.steps().len() {
            self.current_step += 1;
            self.current_group = 0;
            self.selected_option = 0;
        } else {
            self.phase = WizardPhase::Summary;
        }
    }

    /// Go back to previous step
    pub fn previous_step(&mut self) {
        if self.current_step > 0 {
            self.current_step -= 1;
            self.current_group = 0;
            self.selected_option = 0;
        } else if self.phase != WizardPhase::Overview {
            self.phase = WizardPhase::Overview;
        }
    }

    /// Move to next option
    pub fn next_option(&mut self) {
        if let Some(group) = self.current_option_group() {
            if self.selected_option + 1 < group.plugins.plugins.len() {
                self.selected_option += 1;
            }
        }
    }

    /// Move to previous option
    pub fn previous_option(&mut self) {
        if self.selected_option > 0 {
            self.selected_option -= 1;
        }
    }

    /// Move to next group
    pub fn next_group(&mut self) {
        if let Some(step) = self.current_install_step() {
            if self.current_group + 1 < step.groups.groups.len() {
                self.current_group += 1;
                self.selected_option = 0;
            }
        }
    }

    /// Move to previous group
    pub fn previous_group(&mut self) {
        if self.current_group > 0 {
            self.current_group -= 1;
            self.selected_option = 0;
        }
    }
}

impl AppState {
    pub fn new(active_game: Option<Game>) -> Self {
        Self {
            active_game,
            show_help: true,
            browse_limit: 50,
            ..Default::default()
        }
    }

    /// Navigate to a screen
    pub fn goto(&mut self, screen: Screen) {
        self.previous_screen = Some(self.current_screen);
        self.current_screen = screen;
        // Clear status message when navigating to avoid stale messages
        self.status_message = None;
    }

    /// Go back to previous screen
    pub fn go_back(&mut self) {
        if let Some(prev) = self.previous_screen.take() {
            self.current_screen = prev;
        }
    }

    /// Set status message
    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some(msg.into());
    }

    /// Clear status message
    pub fn clear_status(&mut self) {
        self.status_message = None;
    }
}

/// Input mode for text entry
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum InputMode {
    #[default]
    Normal,
    ModInstallPath,
    ProfileNameInput,
    ModDirectoryInput,
    NexusApiKeyInput,
    FomodComponentSelection,
    CollectionPath,
    BrowseSearch,
    PluginPositionInput,
    ModSearch,
    PluginSearch,
}

/// Confirmation dialog
#[derive(Debug, Clone)]
pub struct ConfirmDialog {
    pub title: String,
    pub message: String,
    pub confirm_text: String,
    pub cancel_text: String,
    pub on_confirm: ConfirmAction,
}

/// Actions that can be confirmed
#[derive(Debug, Clone)]
pub enum ConfirmAction {
    DeleteMod(String),
    DeleteProfile(String),
    Deploy,
    Purge,
    // Will be added in Phase 4 when we implement the planner
    // ExecuteFomodPlan(InstallPlan),
}

/// Requirements dialog
#[derive(Debug, Clone)]
pub struct RequirementsDialog {
    pub title: String,
    pub mod_name: String,
    pub missing_mods: Vec<crate::nexus::graphql::ModRequirement>,
    pub dlc_requirements: Vec<crate::nexus::graphql::ModRequirement>,
    pub installed_count: usize,
    pub selected_index: usize,
    pub game_domain: String,
    pub game_id_numeric: i64,
}
