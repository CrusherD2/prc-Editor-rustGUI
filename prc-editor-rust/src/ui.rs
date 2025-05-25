use crate::param_file::ParamFile;
use crate::param_types::*;
use eframe::egui;
use rfd::FileDialog;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub struct PrcEditorApp {
    param_file: ParamFile,
    selected_node: Option<String>, // Path to selected node
    expanded_nodes: HashSet<String>, // Set of expanded node paths
    status_message: String,
    tree_width: f32,
    show_label_editor: bool,
    label_editor_filter: String,
    editing_value: Option<(String, String)>, // (node_path, current_edit_value)
    new_label_input: String, // For adding new labels
    new_hash_input: String, // For adding labels to existing hashes
    label_page: usize, // Current page in label editor
    labels_per_page: usize, // Number of labels per page
    clipboard: Option<String>, // Copied node path
    show_shortcuts_help: bool, // Show shortcuts help dialog
    param_labels_path: Option<String>, // Path to the ParamLabels.csv file
    tree_items: Vec<String>, // Flattened list of visible tree items for navigation
    selected_index: Option<usize>, // Index in tree_items for keyboard navigation
}

impl PrcEditorApp {
    pub fn new() -> Self {
        let mut app = Self {
            param_file: ParamFile::new(),
            selected_node: None,
            expanded_nodes: HashSet::new(),
            status_message: "Ready".to_string(),
            tree_width: 300.0,
            show_label_editor: false,
            label_editor_filter: String::new(),
            editing_value: None,
            new_label_input: String::new(),
            new_hash_input: String::new(),
            label_page: 1,
            labels_per_page: 10,
            clipboard: None,
            show_shortcuts_help: false,
            param_labels_path: None,
            tree_items: Vec::new(),
            selected_index: None,
        };
        
        // Try to load ParamLabels.csv at startup
        app.load_param_labels();
        
        app
    }
    
    fn load_param_labels(&mut self) {
        // First try to load from a previously saved path
        if let Some(saved_path) = self.load_saved_labels_path() {
            if Path::new(&saved_path).exists() {
                match std::fs::read_to_string(&saved_path) {
                    Ok(csv_content) => {
                        self.param_labels_path = Some(saved_path.clone());
                        let file_name = Path::new(&saved_path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("ParamLabels.csv");
                        self.load_labels_from_content(&csv_content, file_name);
                        return;
                    }
                    Err(_) => {
                        self.status_message = format!("Could not read saved ParamLabels.csv at: {}", saved_path);
                    }
                }
            } else {
                self.status_message = format!("Saved ParamLabels.csv path no longer exists: {}", saved_path);
            }
        }
        
        // Try to load from the default location
        if let Ok(csv_content) = std::fs::read_to_string("ParamLabels.csv") {
            self.param_labels_path = Some("ParamLabels.csv".to_string());
            self.load_labels_from_content(&csv_content, "ParamLabels.csv");
            // Save this path for next time
            self.save_labels_path("ParamLabels.csv");
        } else {
            // Try to find it in the Blender addon directory
            if let Some(blender_dir) = Self::find_blender_addon_directory() {
                let mut param_labels_path = blender_dir;
                param_labels_path.push("ParamLabels.csv");
                
                if let Ok(csv_content) = std::fs::read_to_string(&param_labels_path) {
                    let path_string = param_labels_path.to_string_lossy().to_string();
                    self.param_labels_path = Some(path_string.clone());
                    self.load_labels_from_content(&csv_content, "ParamLabels.csv");
                    self.save_labels_path(&path_string);
                    return;
                }
            }
            
            // If not found anywhere, prompt user to select the file (required)
            self.status_message = "ParamLabels.csv is required - please select location".to_string();
            self.prompt_for_labels_file();
        }
    }
    
    fn load_labels_from_content(&mut self, csv_content: &str, file_path: &str) {
        match self.param_file.hash_labels.load_from_csv(csv_content) {
            Ok(count) => {
                self.status_message = format!("Loaded {} param labels from {}", count, file_path);
                // Rebuild the tree to apply the new labels to field names
                self.param_file.rebuild_tree_with_labels();
            }
            Err(e) => {
                self.status_message = format!("Error loading labels from {}: {}", file_path, e);
            }
        }
    }
    
    fn prompt_for_labels_file(&mut self) {
        // Try to find the Blender addon directory as default
        let mut dialog = FileDialog::new()
            .add_filter("CSV files", &["csv"])
            .add_filter("All files", &["*"])
            .set_title("Select ParamLabels.csv file (REQUIRED)")
            .set_file_name("ParamLabels.csv");
        
        // Set default directory to Blender addon if found
        if let Some(blender_dir) = Self::find_blender_addon_directory() {
            dialog = dialog.set_directory(&blender_dir);
            self.status_message = format!("Please select ParamLabels.csv (defaulting to Blender addon directory: {})", blender_dir.display());
        } else {
            self.status_message = "Please select ParamLabels.csv file (Blender addon directory not found)".to_string();
        }
        
        if let Some(file_path) = dialog.pick_file() {
            match std::fs::read_to_string(&file_path) {
                Ok(csv_content) => {
                    // Store the full path for future read/write operations
                    let path_string = file_path.to_string_lossy().to_string();
                    self.param_labels_path = Some(path_string.clone());
                    let file_name = file_path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("selected file");
                    self.load_labels_from_content(&csv_content, file_name);
                    
                    // Save this path for next time
                    self.save_labels_path(&path_string);
                }
                Err(e) => {
                    self.status_message = format!("Error reading selected file: {}", e);
                    // Keep prompting until a valid file is selected
                    self.prompt_for_labels_file();
                }
            }
        } else {
            // Make ParamLabels.csv required - keep prompting until selected
            self.status_message = "ParamLabels.csv is required to use this editor. Please select a valid file.".to_string();
            // We could add a delay here or show a more prominent dialog
        }
    }

    fn show_menu_bar(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                let has_labels = self.param_labels_path.is_some();
                let open_button = ui.add_enabled(has_labels, egui::Button::new("Open"));
                if open_button.clicked() {
                    self.open_file_dialog();
                    ui.close_menu();
                }
                if !has_labels && open_button.hovered() {
                    open_button.on_hover_text("Load ParamLabels.csv first");
                }
                
                ui.separator();
                
                let has_file = self.param_file.get_root().is_some();
                if ui.add_enabled(has_file, egui::Button::new("Save")).clicked() {
                    self.save_file_dialog();
                    ui.close_menu();
                }
                
                if ui.add_enabled(has_file, egui::Button::new("Save As...")).clicked() {
                    self.save_file_dialog();
                    ui.close_menu();
                }
            });

            ui.menu_button("Labels", |ui| {
                if ui.button("Load Labels...").clicked() {
                    self.prompt_for_labels_file();
                    ui.close_menu();
                }
                
                if ui.button("Change Location...").clicked() {
                    self.prompt_for_labels_file();
                    ui.close_menu();
                }
                
                ui.separator();
                
                // Show current labels file path
                if let Some(path) = &self.param_labels_path {
                    let filename = Path::new(path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");
                    ui.label(format!("Current: {}", filename));
                    if ui.small_button("📁").on_hover_text("Show full path").clicked() {
                        self.status_message = format!("ParamLabels.csv location: {}", path);
                    }
                } else {
                    ui.label("No labels file loaded");
                }
                
                ui.separator();
                
                if ui.button("Edit").clicked() {
                    self.show_label_editor = true;
                    ui.close_menu();
                }
                
                if ui.button("Save").clicked() {
                    if let Some(path) = &self.param_labels_path {
                        // Save to the current path
                        match self.param_file.hash_labels.save_to_csv(path) {
                            Ok(()) => {
                                self.status_message = format!("Labels saved to {}", path);
                            }
                            Err(e) => {
                                self.status_message = format!("Error saving labels: {}", e);
                            }
                        }
                    } else {
                        self.status_message = "No labels file path set - use 'Load Labels...' first".to_string();
                    }
                    ui.close_menu();
                }
                
                if ui.button("Download").clicked() {
                    self.download_labels();
                    ui.close_menu();
                }
            });
        });
    }

    fn show_main_content(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
        egui::SidePanel::left("parameter_tree")
            .resizable(true)
            .default_width(self.tree_width)
            .width_range(200.0..=500.0)
            .show_inside(ui, |ui| {
                ui.heading("Parameter Tree");
                ui.separator();
                
                // Build tree items for keyboard navigation before showing the tree
                if self.param_file.get_root().is_some() {
                    self.build_tree_items();
                }
                
                // Make the scroll area fill all available vertical space
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])  // Don't shrink horizontally or vertically
                    .show(ui, |ui| {
                    if self.param_labels_path.is_none() {
                        ui.vertical_centered(|ui| {
                            ui.add_space(20.0);
                            ui.colored_label(egui::Color32::YELLOW, "⚠ ParamLabels.csv Required");
                            ui.add_space(10.0);
                            ui.label("This editor requires ParamLabels.csv to function properly.");
                            ui.add_space(5.0);
                            if ui.button("Select ParamLabels.csv").clicked() {
                                self.prompt_for_labels_file();
                            }
                        });
                    } else if let Some(root) = self.param_file.get_root() {
                        // Clone the root to avoid borrowing issues
                        let root_clone = root.clone();
                        self.show_tree_node(ui, &root_clone, "root".to_string());
                    } else if self.status_message.contains("Error") {
                        ui.colored_label(egui::Color32::LIGHT_RED, "Failed to parse file");
                        ui.label("Check console for details");
                        if ui.button("Try another file").clicked() {
                            self.open_file_dialog();
                        }
                    } else {
                        ui.label("No file loaded");
                        if ui.button("Open file").clicked() {
                            self.open_file_dialog();
                        }
                    }
                });
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("Parameter Details");
            ui.separator();
            
            if let Some(selected_path) = self.selected_node.clone() {
                self.show_parameter_details(ui, &selected_path);
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(100.0);
                    ui.label("Select a parameter to view details");
                });
            }
        });
    }

    fn show_tree_node(&mut self, ui: &mut egui::Ui, node: &ParamNode, path: String) {
        let is_expanded = self.expanded_nodes.contains(&path);
        let is_selected = self.selected_node.as_ref() == Some(&path);
        let is_keyboard_selected = self.selected_index
            .and_then(|idx| self.tree_items.get(idx))
            .map(|selected_path| selected_path == &path)
            .unwrap_or(false);

        // Create the tree node header
        let response = if node.is_expandable() {
            let icon = if is_expanded { "▼" } else { "▶" };
            ui.horizontal(|ui| {
                if ui.button(icon).clicked() {
                    if is_expanded {
                        self.expanded_nodes.remove(&path);
                    } else {
                        self.expanded_nodes.insert(path.clone());
                    }
                }
                
                let type_icon = match &node.value {
                    ParamValue::Struct(_) => "📁",
                    ParamValue::List(_) => "📋",
                    _ => "📄",
                };
                
                ui.label(type_icon);
                
                let label = if node.name.is_empty() || node.name.starts_with("0x") {
                    format!("0x{:X}", node.hash)
                } else {
                    // Truncate long names for tree display
                    if node.name.len() > 25 {
                        format!("{}...", &node.name[..22])
                    } else {
                        node.name.clone()
                    }
                };
                
                let label_response = ui.selectable_label(is_selected || is_keyboard_selected, label);
                
                // Add visual indication for keyboard selection
                if is_keyboard_selected && !is_selected {
                    let rect = label_response.rect;
                    ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(1.0, egui::Color32::YELLOW));
                }
                
                label_response
            }).inner
        } else {
            ui.horizontal(|ui| {
                ui.add_space(20.0); // Indent for leaf nodes
                ui.label("📄");
                
                let label = if node.name.is_empty() || node.name.starts_with("0x") {
                    format!("0x{:X}", node.hash)
                } else {
                    // Truncate long names for tree display
                    if node.name.len() > 20 {
                        format!("{}...", &node.name[..17])
                    } else {
                        node.name.clone()
                    }
                };
                
                // Simplified display for leaf nodes - just name and type
                let value_display = if matches!(node.value, ParamValue::Hash(_)) {
                    node.get_value_string_with_labels(&self.param_file.hash_labels)
                } else {
                    node.get_value_string()
                };
                let display_text = format!("{} ({})", label, node.get_type_name());
                
                let label_response = ui.selectable_label(is_selected || is_keyboard_selected, display_text);
                
                // Add visual indication for keyboard selection
                if is_keyboard_selected && !is_selected {
                    let rect = label_response.rect;
                    ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(1.0, egui::Color32::YELLOW));
                }
                
                label_response
            }).inner
        };

        // Handle selection
        if response.clicked() {
            self.selected_node = Some(path.clone());
        }

        // Show children if expanded
        if is_expanded && node.is_expandable() {
            ui.indent(egui::Id::new(format!("{}_indent", path)), |ui| {
                for (i, child) in node.children.iter().enumerate() {
                    let child_path = format!("{}[{}]", path, i);
                    self.show_tree_node(ui, child, child_path);
                }
            });
        }
    }

    fn show_parameter_details(&mut self, ui: &mut egui::Ui, selected_path: &str) {
        // Parse the path to find the selected node
        if let Some(node) = self.find_node_by_path(selected_path) {
            let node_clone = node.clone(); // Clone to avoid borrowing issues
            
            ui.heading(&format!("Parameter: {}", if node_clone.name.is_empty() { format!("0x{:X}", node_clone.hash) } else { node_clone.name.clone() }));
            ui.separator();
            
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new("param_details")
                    .num_columns(2)
                    .striped(true)
                    .spacing([20.0, 4.0])
                    .show(ui, |ui| {
                        ui.strong("Name:");
                        ui.label(&node_clone.name);
                        ui.end_row();
                        
                        ui.strong("Hash:");
                        ui.monospace(format!("0x{:X}", node_clone.hash));
                        ui.end_row();
                        
                        ui.strong("Type:");
                        ui.label(node_clone.get_type_name());
                        ui.end_row();
                        
                        ui.strong("Value:");
                        ui.monospace(node_clone.get_value_string_with_labels(&self.param_file.hash_labels));
                        ui.end_row();
                        
                        match &node_clone.value {
                            ParamValue::Struct(s) => {
                                ui.strong("Fields:");
                                ui.label(format!("{} fields", s.fields.len()));
                                ui.end_row();
                            }
                            ParamValue::List(l) => {
                                ui.strong("Items:");
                                ui.label(format!("{} items", l.values.len()));
                                ui.end_row();
                            }
                            _ => {}
                        }
                    });
                
                ui.add_space(10.0);
                
                // Show editing interface based on parameter type
                match &node_clone.value {
                    ParamValue::Struct(_) => {
                        self.show_struct_editor(ui, &node_clone, selected_path);
                    }
                    ParamValue::List(_) => {
                        self.show_list_editor(ui, &node_clone, selected_path);
                    }
                    _ => {
                        self.show_value_editor(ui, &node_clone, selected_path);
                    }
                }
            });
        } else {
            ui.label(format!("Could not find node at path: {}", selected_path));
        }
    }
    
    fn show_struct_editor(&mut self, ui: &mut egui::Ui, node: &ParamNode, _selected_path: &str) {
        ui.separator();
        ui.horizontal(|ui| {
            ui.heading("Fields");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("+ Add Field").clicked() {
                    self.status_message = "Add field functionality coming soon".to_string();
                }
            });
        });
        ui.add_space(5.0);
        
        let mut new_editing_value = self.editing_value.clone();
        let mut new_status_message = None;
        
        egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
            egui::Grid::new("struct_fields")
                .num_columns(5)
                .striped(true)
                .spacing([15.0, 6.0])
                .min_col_width(120.0)
                .show(ui, |ui| {
                    ui.strong("Key");
                    ui.strong("Hash");
                    ui.strong("Type");
                    ui.strong("Value");
                    ui.strong("Actions");
                    ui.end_row();
                    
                    for (i, child) in node.children.iter().enumerate() {
                        let child_path = format!("{}[{}]", _selected_path, i);
                        
                        // Key/Name column - editable
                        let key_edit_path = format!("{}_key", child_path);
                        let is_editing_key = new_editing_value.as_ref()
                            .map(|(path, _)| path == &key_edit_path)
                            .unwrap_or(false);
                        
                        if is_editing_key {
                            let mut edit_key = new_editing_value.as_ref().unwrap().1.clone();
                            let response = ui.text_edit_singleline(&mut edit_key);
                            
                            if response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                // Generate hash for new key name
                                let new_hash = self.param_file.hash_labels.add_label_and_save(&edit_key, self.param_labels_path.as_deref());
                                
                                // Actually update the node using the new method
                                if self.param_file.update_node_key(&child_path, edit_key.clone(), new_hash) {
                                    let path_display = self.param_labels_path.as_deref().unwrap_or("ParamLabels.csv");
                                    new_status_message = Some(format!("Key renamed to '{}' (hash: 0x{:X}) and saved to {}", edit_key, new_hash, path_display));
                                    // Refresh tree to show updated keys
                                    // self.refresh_tree();
                                } else {
                                    new_status_message = Some("Failed to update key".to_string());
                                }
                                new_editing_value = None;
                            } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                new_editing_value = None;
                            } else {
                                new_editing_value = Some((key_edit_path.clone(), edit_key));
                            }
                        } else {
                            let display_name = if child.name.len() > 15 {
                                format!("{}...", &child.name[..12])
                            } else {
                                child.name.clone()
                            };
                            
                            let response = ui.add(
                                egui::Label::new(egui::RichText::new(display_name).strong())
                                    .sense(egui::Sense::click())
                            );
                            
                            if response.clicked() {
                                new_editing_value = Some((key_edit_path, child.name.clone()));
                            }
                            
                            if response.hovered() {
                                response.on_hover_text("Click to rename key");
                            }
                        }
                        
                        // Hash column (read-only)
                        ui.monospace(format!("0x{:X}", child.hash));
                        
                        // Type column with dropdown
                        egui::ComboBox::from_id_source(format!("type_{}", i))
                            .selected_text(child.get_type_name())
                            .show_ui(ui, |ui| {
                                let types = ["bool", "sbyte", "byte", "short", "ushort", "int", "uint", "float", "hash40", "string", "list", "struct"];
                                for type_name in types {
                                    if ui.selectable_label(false, type_name).clicked() {
                                        new_status_message = Some(format!("Type changed to {}", type_name));
                                    }
                                }
                            });
                        
                        // Value column
                        let is_editing = new_editing_value.as_ref()
                            .map(|(path, _)| path == &child_path)
                            .unwrap_or(false);
                        
                        if is_editing {
                            let mut edit_value = new_editing_value.as_ref().unwrap().1.clone();
                            let response = ui.text_edit_singleline(&mut edit_value);
                            
                            if response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                // If it's a Hash40 value and looks like a label, generate hash
                                if matches!(child.value, ParamValue::Hash(_)) && !edit_value.starts_with("0x") {
                                    let hash = self.param_file.hash_labels.add_label_and_save(&edit_value, self.param_labels_path.as_deref());
                                    
                                    // Actually update the hash value using the new method
                                    if self.param_file.update_node_value(&child_path, ParamValue::Hash(hash)) {
                                        let path_display = self.param_labels_path.as_deref().unwrap_or("ParamLabels.csv");
                                        new_status_message = Some(format!("Hash40 value set to '{}' (0x{:X}) and saved to {}", edit_value, hash, path_display));
                                        // Refresh tree to show updated values
                                        // self.refresh_tree();
                                    } else {
                                        new_status_message = Some("Failed to update hash value".to_string());
                                    }
                                } else {
                                    // Try to parse the value based on the current type
                                    let updated_value = match &child.value {
                                        ParamValue::Bool(_) => {
                                            if let Ok(val) = edit_value.parse::<bool>() {
                                                Some(ParamValue::Bool(val))
                                            } else if edit_value.to_lowercase() == "true" {
                                                Some(ParamValue::Bool(true))
                                            } else if edit_value.to_lowercase() == "false" {
                                                Some(ParamValue::Bool(false))
                                            } else { None }
                                        }
                                        ParamValue::I8(_) => {
                                            if let Ok(val) = edit_value.parse::<i8>() {
                                                Some(ParamValue::I8(val))
                                            } else { None }
                                        }
                                        ParamValue::U8(_) => {
                                            if let Ok(val) = edit_value.parse::<u8>() {
                                                Some(ParamValue::U8(val))
                                            } else { None }
                                        }
                                        ParamValue::I16(_) => {
                                            if let Ok(val) = edit_value.parse::<i16>() {
                                                Some(ParamValue::I16(val))
                                            } else { None }
                                        }
                                        ParamValue::U16(_) => {
                                            if let Ok(val) = edit_value.parse::<u16>() {
                                                Some(ParamValue::U16(val))
                                            } else { None }
                                        }
                                        ParamValue::I32(_) => {
                                            if let Ok(val) = edit_value.parse::<i32>() {
                                                Some(ParamValue::I32(val))
                                            } else { None }
                                        }
                                        ParamValue::U32(_) => {
                                            if let Ok(val) = edit_value.parse::<u32>() {
                                                Some(ParamValue::U32(val))
                                            } else { None }
                                        }
                                        ParamValue::F32(_) => {
                                            if let Ok(val) = edit_value.parse::<f32>() {
                                                Some(ParamValue::F32(val))
                                            } else { None }
                                        }
                                        ParamValue::String(_) => {
                                            Some(ParamValue::String(edit_value.clone()))
                                        }
                                        ParamValue::Hash(_) => {
                                            if let Ok(val) = u64::from_str_radix(&edit_value.trim_start_matches("0x"), 16) {
                                                Some(ParamValue::Hash(val))
                                            } else { None }
                                        }
                                        _ => None,
                                    };
                                    
                                    if let Some(new_value) = updated_value {
                                        if self.param_file.update_node_value(&child_path, new_value.clone()) {
                                            new_status_message = Some(format!("Value updated to: {}", edit_value));
                                            // Refresh tree to show updated values
                                            // self.refresh_tree();
                                        } else {
                                            new_status_message = Some("Failed to update value".to_string());
                                        }
                                    } else {
                                        new_status_message = Some(format!("Invalid value for type: {}", edit_value));
                                    }
                                }
                                new_editing_value = None;
                            } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                new_editing_value = None;
                            } else {
                                new_editing_value = Some((child_path.clone(), edit_value));
                            }
                        } else {
                            let value_str = child.get_value_string_with_labels(&self.param_file.hash_labels);
                            let display_value = if value_str.len() > 25 {
                                format!("{}...", &value_str[..22])
                            } else {
                                value_str.clone()
                            };
                            
                            let response = ui.add(
                                egui::Label::new(egui::RichText::new(display_value).monospace())
                                    .sense(egui::Sense::click())
                            );
                            
                            if response.clicked() {
                                new_editing_value = Some((child_path.clone(), value_str));
                            }
                            
                            if response.hovered() {
                                response.on_hover_text("Click to edit");
                            }
                        }
                        
                        // Actions column
                        ui.horizontal(|ui| {
                            if ui.small_button("✏").on_hover_text("Edit Value").clicked() {
                                let value_str = child.get_value_string_with_labels(&self.param_file.hash_labels);
                                new_editing_value = Some((child_path.clone(), value_str));
                            }
                            if ui.small_button("🔄").on_hover_text("Rename Key").clicked() {
                                let key_edit_path = format!("{}_key", child_path);
                                new_editing_value = Some((key_edit_path, child.name.clone()));
                            }
                            if ui.small_button("🗑").on_hover_text("Delete").clicked() {
                                new_status_message = Some(format!("Delete field: {}", child.name));
                            }
                        });
                        
                        ui.end_row();
                    }
                });
        });
        
        self.editing_value = new_editing_value;
        if let Some(msg) = new_status_message {
            self.status_message = msg;
        }
    }
    
    fn show_list_editor(&mut self, ui: &mut egui::Ui, node: &ParamNode, _selected_path: &str) {
        ui.separator();
        ui.horizontal(|ui| {
            ui.heading("Items");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("+ Add Item").clicked() {
                    self.status_message = "Add item functionality coming soon".to_string();
                }
            });
        });
        ui.add_space(5.0);
        
        let mut new_editing_value = self.editing_value.clone();
        let mut new_status_message = None;
        
        egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
            egui::Grid::new("list_items")
                .num_columns(4)
                .striped(true)
                .spacing([15.0, 6.0])
                .min_col_width(80.0)
                .show(ui, |ui| {
                    ui.strong("Index");
                    ui.strong("Type");
                    ui.strong("Value");
                    ui.strong("Actions");
                    ui.end_row();
                    
                    for (i, child) in node.children.iter().enumerate() {
                        let child_path = format!("{}[{}]", _selected_path, i);
                        
                        // Index column
                        ui.label(i.to_string());
                        
                        // Type column with dropdown
                        egui::ComboBox::from_id_source(format!("list_type_{}", i))
                            .selected_text(child.get_type_name())
                            .show_ui(ui, |ui| {
                                let types = ["bool", "sbyte", "byte", "short", "ushort", "int", "uint", "float", "hash40", "string", "list", "struct"];
                                for type_name in types {
                                    if ui.selectable_label(false, type_name).clicked() {
                                        new_status_message = Some(format!("Item {} type changed to {}", i, type_name));
                                    }
                                }
                            });
                        
                        // Value column
                        let is_editing = new_editing_value.as_ref()
                            .map(|(path, _)| path == &child_path)
                            .unwrap_or(false);
                        
                        if is_editing {
                            let mut edit_value = new_editing_value.as_ref().unwrap().1.clone();
                            let response = ui.text_edit_singleline(&mut edit_value);
                            
                            if response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                new_status_message = Some(format!("Item {} value edited to: {}", i, edit_value));
                                new_editing_value = None;
                            } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                new_editing_value = None;
                            } else {
                                new_editing_value = Some((child_path.clone(), edit_value));
                            }
                        } else {
                            let value_str = child.get_value_string_with_labels(&self.param_file.hash_labels);
                            let display_value = if value_str.len() > 25 {
                                format!("{}...", &value_str[..22])
                            } else {
                                value_str.clone()
                            };
                            
                            let response = ui.add(
                                egui::Label::new(egui::RichText::new(display_value).monospace())
                                    .sense(egui::Sense::click())
                            );
                            
                            if response.clicked() {
                                new_editing_value = Some((child_path.clone(), value_str));
                            }
                            
                            if response.hovered() {
                                response.on_hover_text("Click to edit");
                            }
                        }
                        
                        // Actions column
                        ui.horizontal(|ui| {
                            if ui.small_button("✏").on_hover_text("Edit").clicked() {
                                let value_str = child.get_value_string_with_labels(&self.param_file.hash_labels);
                                new_editing_value = Some((child_path.clone(), value_str));
                            }
                            if ui.small_button("🗑").on_hover_text("Delete").clicked() {
                                new_status_message = Some(format!("Delete item {}", i));
                            }
                        });
                        
                        ui.end_row();
                    }
                });
        });
        
        self.editing_value = new_editing_value;
        if let Some(msg) = new_status_message {
            self.status_message = msg;
        }
    }
    
    fn show_value_editor(&mut self, ui: &mut egui::Ui, node: &ParamNode, selected_path: &str) {
        ui.separator();
        ui.heading("Edit Value");
        ui.add_space(5.0);
        
        let mut new_editing_value = self.editing_value.clone();
        let mut new_status_message = None;
        
        egui::Grid::new("value_editor")
            .num_columns(3)
            .striped(false)
            .spacing([15.0, 8.0])
            .show(ui, |ui| {
                ui.strong("Type:");
                
                // Type dropdown
                egui::ComboBox::from_id_source("value_type")
                    .selected_text(node.get_type_name())
                    .show_ui(ui, |ui| {
                        let types = ["bool", "sbyte", "byte", "short", "ushort", "int", "uint", "float", "hash40", "string", "list", "struct"];
                        for type_name in types {
                            if ui.selectable_label(false, type_name).clicked() {
                                new_status_message = Some(format!("Type changed to {}", type_name));
                            }
                        }
                    });
                
                ui.label(""); // Empty third column
                ui.end_row();
                
                ui.strong("Value:");
                
                // Value editor
                let is_editing = new_editing_value.as_ref()
                    .map(|(path, _)| path == selected_path)
                    .unwrap_or(false);
                
                if is_editing {
                    let mut edit_value = new_editing_value.as_ref().unwrap().1.clone();
                    let response = ui.text_edit_singleline(&mut edit_value);
                    
                    if response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        new_status_message = Some(format!("Value saved: {}", edit_value));
                        new_editing_value = None;
                    } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        new_editing_value = None;
                    } else {
                        new_editing_value = Some((selected_path.to_string(), edit_value));
                    }
                } else {
                    let value_str = node.get_value_string_with_labels(&self.param_file.hash_labels);
                    let response = ui.add(
                        egui::Label::new(egui::RichText::new(&value_str).monospace())
                            .sense(egui::Sense::click())
                    );
                    
                    if response.clicked() {
                        new_editing_value = Some((selected_path.to_string(), value_str));
                    }
                    
                    if response.hovered() {
                        response.on_hover_text("Click to edit");
                    }
                }
                
                // Edit button
                if ui.button("Edit").clicked() {
                    let value_str = node.get_value_string_with_labels(&self.param_file.hash_labels);
                    new_editing_value = Some((selected_path.to_string(), value_str));
                }
                
                ui.end_row();
            });
        
        self.editing_value = new_editing_value;
        if let Some(msg) = new_status_message {
            self.status_message = msg;
        }
    }
    
    fn find_node_by_path(&self, path: &str) -> Option<&ParamNode> {
        let root = self.param_file.get_root()?;
        
        if path == "root" {
            return Some(root);
        }
        
        // Parse path like "root[0][1][2]" to navigate to the correct node
        let mut current_node = root;
        let path_parts: Vec<&str> = path.split("[").skip(1).collect(); // Skip "root" part
        
        for part in path_parts {
            let index_str = part.trim_end_matches(']');
            if let Ok(index) = index_str.parse::<usize>() {
                if index < current_node.children.len() {
                    current_node = &current_node.children[index];
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
        
        Some(current_node)
    }

    /// Refresh the tree display from the underlying data structure
    /// This should be called after making changes to ensure UI consistency
    fn refresh_tree(&mut self) {
        if let Some(_) = self.param_file.get_root() {
            // Preserve expanded state and selection
            let expanded_nodes = self.expanded_nodes.clone();
            let selected_path = self.selected_node.clone();
            
            // Rebuild the tree from the underlying data structure
            self.param_file.rebuild_tree();
            
            // Restore expanded state
            self.expanded_nodes = expanded_nodes;
            
            // Try to restore selection if it still exists
            if let Some(path) = selected_path {
                if self.find_node_by_path(&path).is_some() {
                    self.selected_node = Some(path);
                } else {
                    self.selected_node = None;
                }
            }
        }
    }

    fn open_file_dialog(&mut self) {
        // Check if ParamLabels.csv is loaded first
        if self.param_labels_path.is_none() {
            self.status_message = "Please load ParamLabels.csv first before opening parameter files".to_string();
            self.prompt_for_labels_file();
            return;
        }
        
        if let Some(file_path) = FileDialog::new()
            .add_filter("Param files", &["prc", "prcx", "stdat", "stdatx", "stprm", "stprmx"])
            .add_filter("All files", &["*"])
            .pick_file()
        {
            self.status_message = format!("Opening file: {}", file_path.display());
            
            match std::fs::read(&file_path) {
                Ok(data) => {
                    let filename = file_path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");
                    
                    match self.param_file.open(&data, filename) {
                        Ok(()) => {
                            self.status_message = format!("Successfully opened: {}", filename);
                            self.selected_node = None;
                            self.expanded_nodes.clear();
                            // Rebuild tree with labels if they're already loaded
                            if !self.param_file.hash_labels.is_empty() {
                                self.param_file.rebuild_tree_with_labels();
                            }
                        }
                        Err(e) => {
                            eprintln!("Detailed error: {:#?}", e);
                            self.status_message = format!("Error opening file: {}", e);
                            // Clear any partial data
                            self.param_file.root = None;
                        }
                    }
                }
                Err(e) => {
                    self.status_message = format!("Error reading file: {}", e);
                }
            }
        }
    }

    fn save_file_dialog(&mut self) {
        if let Some(file_path) = FileDialog::new()
            .add_filter("Param files", &["prc", "prcx", "stdat", "stdatx", "stprm", "stprmx"])
            .add_filter("All files", &["*"])
            .set_file_name(&format!("{}_modified", self.param_file.get_filename().replace(".prc", "")))
            .save_file()
        {
            self.status_message = format!("Saving file: {}", file_path.display());
            
            match self.param_file.save(file_path.to_str().unwrap_or("output.prc")) {
                Ok(()) => {
                    self.status_message = format!("Successfully saved: {}", file_path.display());
                }
                Err(e) => {
                    eprintln!("Save error: {:#?}", e);
                    self.status_message = format!("Error saving file: {}", e);
                }
            }
        }
    }

    fn download_labels(&mut self) {
        // TODO: Implement label downloading from online source
        self.status_message = "Label downloading not yet implemented".to_string();
    }
    
    /// Build a flattened list of visible tree items for keyboard navigation
    fn build_tree_items(&mut self) {
        self.tree_items.clear();
        if let Some(root) = self.param_file.get_root() {
            self.collect_visible_items(&root.clone(), "root".to_string(), 0);
        }
        
        // Update selected_index to match selected_node
        if let Some(selected_path) = &self.selected_node {
            self.selected_index = self.tree_items.iter().position(|item| item == selected_path);
        } else if !self.tree_items.is_empty() {
            // If no selection, select the first item
            self.selected_index = Some(0);
            self.selected_node = self.tree_items.first().cloned();
        }
    }
    
    /// Recursively collect visible tree items
    fn collect_visible_items(&mut self, node: &ParamNode, path: String, _depth: usize) {
        self.tree_items.push(path.clone());
        
        // Only collect children if this node is expanded
        if node.is_expandable() && self.expanded_nodes.contains(&path) {
            for (i, child) in node.children.iter().enumerate() {
                let child_path = format!("{}[{}]", path, i);
                self.collect_visible_items(child, child_path, _depth + 1);
            }
        }
    }
    
    /// Navigate up in the tree
    fn navigate_up(&mut self) {
        if let Some(current_index) = self.selected_index {
            if current_index > 0 {
                self.selected_index = Some(current_index - 1);
                if let Some(new_path) = self.tree_items.get(current_index - 1) {
                    self.selected_node = Some(new_path.clone());
                }
            }
        } else if !self.tree_items.is_empty() {
            self.selected_index = Some(0);
            self.selected_node = self.tree_items.first().cloned();
        }
    }
    
    /// Navigate down in the tree
    fn navigate_down(&mut self) {
        if let Some(current_index) = self.selected_index {
            if current_index + 1 < self.tree_items.len() {
                self.selected_index = Some(current_index + 1);
                if let Some(new_path) = self.tree_items.get(current_index + 1) {
                    self.selected_node = Some(new_path.clone());
                }
            }
        } else if !self.tree_items.is_empty() {
            self.selected_index = Some(0);
            self.selected_node = self.tree_items.first().cloned();
        }
    }
    
    /// Navigate left (collapse current node or go to parent)
    fn navigate_left(&mut self) {
        if let Some(selected_path) = &self.selected_node.clone() {
            // If current node is expanded, collapse it
            if self.expanded_nodes.contains(selected_path) {
                self.expanded_nodes.remove(selected_path);
                // Rebuild tree items since visibility changed
                self.build_tree_items();
            } else {
                // Go to parent node
                if let Some(parent_path) = self.get_parent_path(selected_path) {
                    self.selected_node = Some(parent_path.clone());
                    self.selected_index = self.tree_items.iter().position(|item| item == &parent_path);
                }
            }
        }
    }
    
    /// Navigate right (expand current node or go to first child)
    fn navigate_right(&mut self) {
        if let Some(selected_path) = &self.selected_node.clone() {
            if let Some(node) = self.find_node_by_path(selected_path) {
                if node.is_expandable() {
                    if !self.expanded_nodes.contains(selected_path) {
                        // Expand the node
                        self.expanded_nodes.insert(selected_path.clone());
                        // Rebuild tree items since visibility changed
                        self.build_tree_items();
                    } else if !node.children.is_empty() {
                        // Go to first child
                        let first_child_path = format!("{}[0]", selected_path);
                        self.selected_node = Some(first_child_path.clone());
                        self.selected_index = self.tree_items.iter().position(|item| item == &first_child_path);
                    }
                }
            }
        }
    }
    
    /// Get the parent path of a given path
    fn get_parent_path(&self, path: &str) -> Option<String> {
        if path == "root" {
            return None;
        }
        
        // Find the last '[' and remove everything from there
        if let Some(last_bracket) = path.rfind('[') {
            Some(path[..last_bracket].to_string())
        } else {
            None
        }
    }
    
    /// Find the Smash Ultimate Blender plugin directory
    fn find_blender_addon_directory() -> Option<PathBuf> {
        // Get the user's AppData/Roaming directory
        if let Some(mut appdata_dir) = dirs::config_dir() {
            // On Windows, this gives us AppData/Roaming
            // On other platforms, we'll try to find Blender config
            
            #[cfg(target_os = "windows")]
            {
                appdata_dir.push("Blender Foundation");
                appdata_dir.push("Blender");
            }
            
            #[cfg(not(target_os = "windows"))]
            {
                // On Linux/Mac, Blender config is usually in ~/.config/blender/
                appdata_dir.push("blender");
            }
            
            // Try to find any Blender version directory
            if let Ok(entries) = std::fs::read_dir(&appdata_dir) {
                let mut blender_versions: Vec<_> = entries
                    .filter_map(|entry| entry.ok())
                    .filter(|entry| entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
                    .filter_map(|entry| {
                        let name = entry.file_name().to_string_lossy().to_string();
                        // Look for version patterns like "4.2", "3.6", etc.
                        if name.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                            Some((name, entry.path()))
                        } else {
                            None
                        }
                    })
                    .collect();
                
                // Sort by version (newest first)
                blender_versions.sort_by(|a, b| b.0.cmp(&a.0));
                
                // Try each version directory
                for (_, version_path) in blender_versions {
                    let mut addon_path = version_path;
                    addon_path.push("scripts");
                    addon_path.push("addons");
                    addon_path.push("smash_ultimate_blender");
                    addon_path.push("dependencies");
                    addon_path.push("pyprc");
                    
                    if addon_path.exists() {
                        return Some(addon_path);
                    }
                }
            }
        }
        
        None
    }
    
    /// Get the path to the configuration file
    fn get_config_path() -> std::path::PathBuf {
        // Store config in the same directory as the executable
        let mut config_path = std::env::current_exe()
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        config_path.pop(); // Remove the executable name
        config_path.push("prc_editor_config.txt");
        config_path
    }
    
    /// Save the ParamLabels.csv path to a config file
    fn save_labels_path(&self, path: &str) {
        let config_path = Self::get_config_path();
        if let Err(e) = std::fs::write(&config_path, path) {
            eprintln!("Warning: Could not save config to {}: {}", config_path.display(), e);
        }
    }
    
    /// Load the saved ParamLabels.csv path from the config file
    fn load_saved_labels_path(&self) -> Option<String> {
        let config_path = Self::get_config_path();
        match std::fs::read_to_string(&config_path) {
            Ok(content) => {
                let path = content.trim().to_string();
                if !path.is_empty() {
                    Some(path)
                } else {
                    None
                }
            }
            Err(_) => None, // Config file doesn't exist yet
        }
    }
    
    fn show_label_editor_window(&mut self, ctx: &egui::Context) {
        if !self.show_label_editor {
            return;
        }
        
        let mut open = true; // Track if window should stay open
        
        egui::Window::new("Label Editor")
            .default_size([800.0, 600.0])
            .open(&mut open) // This adds the close button (X)
            .show(ctx, |ui| {
                ui.heading("Parameter Labels");
                ui.separator();
                
                // Add new label section
                ui.horizontal(|ui| {
                    ui.label("Add new label:");
                    if ui.text_edit_singleline(&mut self.new_label_input).lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        if !self.new_label_input.is_empty() {
                            let hash = self.param_file.hash_labels.add_label_and_save(&self.new_label_input, self.param_labels_path.as_deref());
                            let path_display = self.param_labels_path.as_deref().unwrap_or("ParamLabels.csv");
                            self.status_message = format!("Added label '{}' with hash 0x{:X} and saved to {}", self.new_label_input, hash, path_display);
                            self.new_label_input.clear();
                            // Rebuild tree to show updated labels
                            self.param_file.rebuild_tree_with_labels();
                        }
                    }
                    if ui.button("Generate Hash").clicked() {
                        if !self.new_label_input.is_empty() {
                            let hash = self.param_file.hash_labels.add_label_and_save(&self.new_label_input, self.param_labels_path.as_deref());
                            let path_display = self.param_labels_path.as_deref().unwrap_or("ParamLabels.csv");
                            self.status_message = format!("Added label '{}' with hash 0x{:X} and saved to {}", self.new_label_input, hash, path_display);
                            self.new_label_input.clear();
                            // Rebuild tree to show updated labels
                            self.param_file.rebuild_tree_with_labels();
                        }
                    }
                });
                
                ui.separator();
                
                // Add label to existing hash section
                ui.horizontal(|ui| {
                    ui.label("Add label to existing hash:");
                    ui.text_edit_singleline(&mut self.new_hash_input);
                    ui.text_edit_singleline(&mut self.new_label_input);
                    if ui.button("Set Label").clicked() {
                        if !self.new_hash_input.is_empty() && !self.new_label_input.is_empty() {
                            // Try to parse the hash
                            let hash_str = self.new_hash_input.trim_start_matches("0x");
                            if let Ok(hash) = u64::from_str_radix(hash_str, 16) {
                                // Add the label for this specific hash and save
                                match self.param_file.hash_labels.add_label_for_hash_and_save(hash, &self.new_label_input, self.param_labels_path.as_deref()) {
                                    Ok(()) => {
                                        let path_display = self.param_labels_path.as_deref().unwrap_or("ParamLabels.csv");
                                        self.status_message = format!("Added label '{}' for hash 0x{:X} and saved to {}", self.new_label_input, hash, path_display);
                                        // Rebuild tree to show updated labels
                                        self.param_file.rebuild_tree_with_labels();
                                    }
                                    Err(e) => {
                                        self.status_message = format!("Added label but failed to save: {}", e);
                                    }
                                }
                                
                                self.new_hash_input.clear();
                                self.new_label_input.clear();
                            } else {
                                self.status_message = "Invalid hash format. Use hex format like 0x1133BC6DD8".to_string();
                            }
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Format: Hash (0x1133BC6DD8) + Label name");
                });
                
                // Filter input and pagination controls
                ui.horizontal(|ui| {
                    ui.label("Filter:");
                    ui.text_edit_singleline(&mut self.label_editor_filter);
                    if ui.button("Clear").clicked() {
                        self.label_editor_filter.clear();
                        self.label_page = 1; // Reset to first page
                    }
                    
                    ui.separator();
                    
                    // Pagination controls
                    let filtered_labels = self.param_file.hash_labels.get_labels_filtered(&self.label_editor_filter);
                    let total_labels = filtered_labels.len();
                    let total_pages = (total_labels + self.labels_per_page - 1) / self.labels_per_page;
                    
                    ui.label(format!("Page {} of {} ({} labels)", self.label_page, total_pages.max(1), total_labels));
                    
                    if self.label_page > 1 && ui.button("◀ Prev").clicked() {
                        self.label_page -= 1;
                    }
                    if self.label_page < total_pages && ui.button("Next ▶").clicked() {
                        self.label_page += 1;
                    }
                    
                    // Page size selector
                    ui.separator();
                    ui.label("Per page:");
                    egui::ComboBox::from_id_source("page_size")
                        .selected_text(self.labels_per_page.to_string())
                        .show_ui(ui, |ui| {
                            for &size in &[10, 25, 50, 100, 250] {
                                if ui.selectable_label(self.labels_per_page == size, size.to_string()).clicked() {
                                    self.labels_per_page = size;
                                    self.label_page = 1; // Reset to first page
                                }
                            }
                        });
                });
                
                ui.separator();
                
                // Labels list
                egui::ScrollArea::vertical().show(ui, |ui| {
                    egui::Grid::new("labels_grid")
                        .num_columns(3)
                        .striped(true)
                        .spacing([15.0, 4.0])
                        .min_col_width(150.0)
                        .show(ui, |ui| {
                            ui.strong("Hash");
                            ui.strong("Label");
                            ui.strong("Actions");
                            ui.end_row();
                            
                            // Show actual loaded labels with pagination
                            let mut filtered_labels = self.param_file.hash_labels.get_labels_filtered(&self.label_editor_filter);
                            filtered_labels.sort_by(|a, b| a.1.cmp(b.1)); // Sort by label name
                            
                            // Calculate pagination
                            let start_index = (self.label_page - 1) * self.labels_per_page;
                            let _end_index = (start_index + self.labels_per_page).min(filtered_labels.len());
                            
                            // Show only current page
                            for (hash, label) in filtered_labels.iter().skip(start_index).take(self.labels_per_page) {
                                ui.monospace(format!("0x{:X}", hash));
                                
                                // Editable label
                                let mut edit_label = (*label).clone();
                                if ui.text_edit_singleline(&mut edit_label).changed() {
                                    // Update the label (this would require more state management in a real app)
                                    self.status_message = format!("Label editing: {}", edit_label);
                                }
                                
                                // Actions
                                ui.horizontal(|ui| {
                                    if ui.small_button("Copy").clicked() {
                                        ui.output_mut(|o| o.copied_text = label.to_string());
                                        self.status_message = format!("Copied: {}", label);
                                    }
                                    if ui.small_button("Delete").clicked() {
                                        self.status_message = format!("Delete label: {}", label);
                                    }
                                });
                                
                                ui.end_row();
                            }
                        });
                });
                
                ui.separator();
                
                ui.horizontal(|ui| {
                    if ui.button("Close").clicked() {
                        self.show_label_editor = false;
                    }
                });
            });
            
        // Handle window close button (X)
        if !open {
            self.show_label_editor = false;
        }
    }

    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            // Only handle shortcuts if no text editing is active
            if !self.editing_value.is_some() && !self.show_label_editor {
                let ctrl = i.modifiers.ctrl;
                
                // Arrow key navigation
                if i.key_pressed(egui::Key::ArrowUp) {
                    self.navigate_up();
                }
                if i.key_pressed(egui::Key::ArrowDown) {
                    self.navigate_down();
                }
                if i.key_pressed(egui::Key::ArrowLeft) {
                    self.navigate_left();
                }
                if i.key_pressed(egui::Key::ArrowRight) {
                    self.navigate_right();
                }
                
                // ENTER - Open data grid for selected node (expand/collapse)
                if i.key_pressed(egui::Key::Enter) {
                    if let Some(selected_path) = &self.selected_node {
                        if self.expanded_nodes.contains(selected_path) {
                            self.expanded_nodes.remove(selected_path);
                            self.status_message = "Collapsed node".to_string();
                        } else {
                            self.expanded_nodes.insert(selected_path.clone());
                            self.status_message = "Expanded node".to_string();
                        }
                    }
                }
                
                // DEL - Delete the node
                if i.key_pressed(egui::Key::Delete) {
                    if let Some(selected_path) = &self.selected_node {
                        self.status_message = format!("Delete node: {} (not implemented)", selected_path);
                    }
                }
                
                // CTRL + C - Copy the node
                if ctrl && i.key_pressed(egui::Key::C) {
                    if let Some(selected_path) = &self.selected_node {
                        self.clipboard = Some(selected_path.clone());
                        self.status_message = format!("Copied node: {}", selected_path);
                    }
                }
                
                // CTRL + X - Cut the node
                if ctrl && i.key_pressed(egui::Key::X) {
                    if let Some(selected_path) = &self.selected_node {
                        self.clipboard = Some(selected_path.clone());
                        self.status_message = format!("Cut node: {} (not implemented)", selected_path);
                    }
                }
                
                // CTRL + V - Paste the copied node into the node
                if ctrl && i.key_pressed(egui::Key::V) {
                    if let (Some(clipboard_path), Some(selected_path)) = (&self.clipboard, &self.selected_node) {
                        self.status_message = format!("Paste {} into {} (not implemented)", clipboard_path, selected_path);
                    } else {
                        self.status_message = "Nothing to paste".to_string();
                    }
                }
                
                // CTRL + P - Paste the copied node into the parent
                if ctrl && i.key_pressed(egui::Key::P) {
                    if let (Some(clipboard_path), Some(selected_path)) = (&self.clipboard, &self.selected_node) {
                        self.status_message = format!("Paste {} into parent of {} (not implemented)", clipboard_path, selected_path);
                    } else {
                        self.status_message = "Nothing to paste into parent".to_string();
                    }
                }
                
                // CTRL + D - Duplicate param on the same level
                if ctrl && i.key_pressed(egui::Key::D) {
                    if let Some(selected_path) = &self.selected_node {
                        self.status_message = format!("Duplicate node: {} (not implemented)", selected_path);
                    }
                }
                
                // CTRL + S - Save file
                if ctrl && i.key_pressed(egui::Key::S) {
                    if self.param_file.get_root().is_some() {
                        self.save_file_dialog();
                    } else {
                        self.status_message = "No file to save".to_string();
                    }
                }
                
                // F1 or ? - Show shortcuts help
                if i.key_pressed(egui::Key::F1) {
                    self.show_shortcuts_help = true;
                }
            }
        });
    }

    fn show_shortcuts_help_window(&mut self, ctx: &egui::Context) {
        if !self.show_shortcuts_help {
            return;
        }
        
        let mut open = true;
        
        egui::Window::new("Tree Shortcuts")
            .default_size([400.0, 300.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.heading("Keyboard Shortcuts");
                ui.separator();
                
                egui::Grid::new("shortcuts_grid")
                    .num_columns(2)
                    .striped(true)
                    .spacing([15.0, 8.0])
                    .show(ui, |ui| {
                        ui.strong("Key");
                        ui.strong("Action");
                        ui.end_row();
                        
                        ui.monospace("↑/↓");
                        ui.label("Navigate up/down in the tree");
                        ui.end_row();
                        
                        ui.monospace("←/→");
                        ui.label("Collapse/expand nodes or navigate to parent/child");
                        ui.end_row();
                        
                        ui.monospace("ENTER");
                        ui.label("Open a data grid for the selected node");
                        ui.end_row();
                        
                        ui.monospace("DEL");
                        ui.label("Delete the node");
                        ui.end_row();
                        
                        ui.monospace("CTRL + C");
                        ui.label("Copy the node");
                        ui.end_row();
                        
                        ui.monospace("CTRL + X");
                        ui.label("Cut the node");
                        ui.end_row();
                        
                        ui.monospace("CTRL + V");
                        ui.label("Paste the copied node into the node");
                        ui.end_row();
                        
                        ui.monospace("CTRL + P");
                        ui.label("Paste the copied node into the parent");
                        ui.end_row();
                        
                        ui.monospace("CTRL + D");
                        ui.label("Duplicate param on the same level");
                        ui.end_row();
                        
                        ui.monospace("CTRL + S");
                        ui.label("Save the file");
                        ui.end_row();
                        
                        ui.monospace("F1");
                        ui.label("Show this help");
                        ui.end_row();
                    });
                
                ui.separator();
                
                ui.horizontal(|ui| {
                    if ui.button("Close").clicked() {
                        self.show_shortcuts_help = false;
                    }
                });
            });
            
        if !open {
            self.show_shortcuts_help = false;
        }
    }
}

impl eframe::App for PrcEditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle keyboard shortcuts
        self.handle_keyboard_shortcuts(ctx);
        
        egui::CentralPanel::default().show(ctx, |ui| {
            // Menu bar
            self.show_menu_bar(ctx, ui);
            
            ui.separator();
            
            // Main content area
            self.show_main_content(ctx, ui);
        });
        
        // Status bar at bottom using bottom panel
        egui::TopBottomPanel::bottom("status_panel")
            .resizable(false)
            .min_height(25.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Status:");
                    ui.label(&self.status_message);
                    
                    // Show shortcuts button
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Shortcuts").clicked() {
                            self.show_shortcuts_help = true;
                        }
                        
                        // Show labels count and file path
                        if let Some(path) = &self.param_labels_path {
                            let filename = std::path::Path::new(path)
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("unknown");
                            ui.label(&format!("Labels: {} ({})", self.param_file.hash_labels.len(), filename));
                        } else {
                            ui.label(&format!("Labels: {} (no file)", self.param_file.hash_labels.len()));
                        }
                    });
                });
            });
        
        // Show label editor window if open
        self.show_label_editor_window(ctx);
        
        // Show shortcuts help if open
        self.show_shortcuts_help_window(ctx);
    }
} 