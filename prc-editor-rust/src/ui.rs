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
    clipboard_data: Option<ParamNode>, // Actual copied node data
    cut_mode: bool, // Whether the clipboard operation was cut (vs copy)
    // show_shortcuts_help removed - shortcuts are now always visible
    param_labels_path: Option<String>, // Path to the ParamLabels.csv file
    tree_items: Vec<String>, // Flattened list of visible tree items for navigation
    selected_index: Option<usize>, // Index in tree_items for keyboard navigation
    undo_stack: Vec<UndoAction>, // Stack of undo actions
    redo_stack: Vec<UndoAction>, // Stack of redo actions
}

#[derive(Clone)]
enum UndoAction {
    DeleteNode {
        path: String,
        node: ParamNode,
        parent_path: String,
        index: usize,
    },
    AddNode {
        path: String,
    },
    UpdateValue {
        path: String,
        old_value: ParamValue,
        new_value: ParamValue,
    },
    UpdateKey {
        path: String,
        old_name: String,
        old_hash: u64,
        new_name: String,
        new_hash: u64,
    },
}

impl PrcEditorApp {
    pub fn new() -> Self {
        let mut app = Self {
            param_file: ParamFile::new(),
            selected_node: None,
            expanded_nodes: HashSet::new(),
            status_message: "Ready".to_string(),
            tree_width: 700.0,
            show_label_editor: false,
            label_editor_filter: String::new(),
            editing_value: None,
            new_label_input: String::new(),
            new_hash_input: String::new(),
            label_page: 1,
            labels_per_page: 10,
            clipboard: None,
            clipboard_data: None,
            cut_mode: false,
            // show_shortcuts_help removed
            param_labels_path: None,
            tree_items: Vec::new(),
            selected_index: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
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

    fn show_main_content(&mut self, ui: &mut egui::Ui) {
        egui::SidePanel::left("parameter_tree")
            .resizable(true)
            .default_width(self.tree_width)
            .min_width(200.0)
            .show_inside(ui, |ui| {
                ui.heading("Parameter Tree");
                ui.separator();
                
                // Build tree items for keyboard navigation before showing the tree
                if self.param_file.get_root().is_some() {
                    self.build_tree_items();
                }
                
                // Make the scroll area use all available space
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])  // Don't shrink in either direction
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
            
            // Main content area with shortcuts overlay
            
            if let Some(selected_path) = self.selected_node.clone() {
                self.show_parameter_details(ui, &selected_path);
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(100.0);
                    ui.label("Select a parameter to view details");
                });
            }
            
            // Add shortcuts box as overlay in absolute bottom-right corner
            let shortcuts_box_width = 280.0;
            let shortcuts_box_height = 200.0;
            
            // Use the UI's clip rect to get the actual drawable area and move closer to corner
            let clip_rect = ui.clip_rect();
            let shortcuts_pos = egui::pos2(
                clip_rect.max.x - shortcuts_box_width - 5.0, // Move 5 pixels away from right edge (inward)
                clip_rect.max.y - shortcuts_box_height - 5.0 // Move 5 pixels away from bottom edge (inward)
            );
            
            // Draw the shortcuts box as overlay (non-interactive background element)
            ui.allocate_ui_at_rect(
                egui::Rect::from_min_size(shortcuts_pos, egui::vec2(shortcuts_box_width, shortcuts_box_height)),
                |ui| {
                    // Background frame
                    let frame = egui::Frame::default()
                        .fill(egui::Color32::from_rgba_unmultiplied(40, 40, 40, 200))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(80, 80, 80, 150)))
                        .rounding(egui::Rounding::same(8.0))
                        .inner_margin(egui::Margin::same(12.0));
                    
                    frame.show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.colored_label(
                                egui::Color32::from_rgba_unmultiplied(200, 200, 200, 255),
                                egui::RichText::new("Keyboard Shortcuts").size(14.0).strong()
                            );
                            ui.add_space(8.0);
                            
                            let shortcuts = [
                                ("↑↓←→", "Navigate tree"),
                                ("Enter", "Expand/collapse"),
                                ("F2", "Rename node"),
                                ("Del", "Delete node"),
                                ("Ctrl+C", "Copy node"),
                                ("Ctrl+X", "Cut node"),
                                ("Ctrl+V", "Paste node"),
                                ("Ctrl+P", "Paste to parent"),
                                ("Ctrl+D", "Duplicate node"),
                                ("Ctrl+S", "Save file"),
                                ("Ctrl+Z", "Undo"),
                                ("Ctrl+Y", "Redo"),
                            ];
                            
                            for (key, desc) in shortcuts {
                                ui.horizontal(|ui| {
                                    ui.colored_label(
                                        egui::Color32::from_rgba_unmultiplied(150, 200, 255, 255),
                                        egui::RichText::new(key).size(11.0).monospace()
                                    );
                                    ui.colored_label(
                                        egui::Color32::from_rgba_unmultiplied(180, 180, 180, 255),
                                        egui::RichText::new(desc).size(11.0)
                                    );
                                });
                            }
                        });
                    });
                }
            );
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
                let _value_display = if matches!(node.value, ParamValue::Hash(_)) {
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
                        
                        // Make name editable
                        let name_edit_path = format!("{}_name", selected_path);
                        let is_editing_name = self.editing_value.as_ref()
                            .map(|(path, _)| path == &name_edit_path)
                            .unwrap_or(false);
                        
                        if is_editing_name {
                            let mut edit_name = self.editing_value.as_ref().unwrap().1.clone();
                            let response = ui.text_edit_singleline(&mut edit_name);
                            
                            if response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                // Get the current node name to check if it actually changed
                                let current_name = if let Some(current_node) = self.find_node_by_path(selected_path) {
                                    current_node.name.clone()
                                } else {
                                    String::new()
                                };
                                
                                // Only proceed if the name actually changed
                                if edit_name != current_name {
                                    // Check for duplicate names and add failsafe
                                    let final_name = self.ensure_unique_name(selected_path, &edit_name);
                                    
                                    // Generate hash for new name
                                    let new_hash = self.param_file.hash_labels.add_label_and_save(&final_name, self.param_labels_path.as_deref());
                                    
                                    // Update the node name and hash
                                    if self.update_node_key_with_undo(selected_path, final_name.clone(), new_hash) {
                                        let path_display = self.param_labels_path.as_deref().unwrap_or("ParamLabels.csv");
                                        let message = if final_name != edit_name {
                                            format!("Node renamed to '{}' (was duplicate, added suffix) (hash: 0x{:X}) and saved to {}", final_name, new_hash, path_display)
                                        } else {
                                            format!("Node renamed to '{}' (hash: 0x{:X}) and saved to {}", final_name, new_hash, path_display)
                                        };
                                        self.status_message = message;
                                        // Rebuild tree to show updated name
                                        self.param_file.rebuild_tree_with_labels();
                                    } else {
                                        self.status_message = "Failed to update node name".to_string();
                                    }
                                } else {
                                    // Name didn't change, just show a message
                                    self.status_message = "Name unchanged".to_string();
                                }
                                self.editing_value = None;
                            } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                self.editing_value = None;
                            } else {
                                self.editing_value = Some((name_edit_path.clone(), edit_name));
                            }
                        } else {
                            let response = ui.add(
                                egui::Label::new(egui::RichText::new(&node_clone.name).strong())
                                    .sense(egui::Sense::click())
                            );
                            
                            if response.clicked() {
                                self.editing_value = Some((name_edit_path, node_clone.name.clone()));
                            }
                            
                            if response.hovered() {
                                response.on_hover_text("Click to rename");
                            }
                        }
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
                                
                                // Actually update the node using the new method with undo tracking
                                if self.update_node_key_with_undo(&child_path, edit_key.clone(), new_hash) {
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
                                    
                                    // Actually update the hash value using the new method with undo tracking
                                    if self.update_node_value_with_undo(&child_path, ParamValue::Hash(hash)) {
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
                                        if self.update_node_value_with_undo(&child_path, new_value.clone()) {
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
    
    /// Delete a node at the given path
    fn delete_node(&mut self, path: &str) -> bool {
        // Cannot delete root
        if path == "root" {
            return false;
        }
        
        let indices = match self.param_file.parse_node_path(path) {
            Some(indices) => indices,
            None => return false,
        };
        
        if indices.is_empty() {
            return false; // Cannot delete root
        }
        
        // Get the node to delete for undo purposes
        let node_to_delete = match self.find_node_by_path(path) {
            Some(node) => node.clone(),
            None => return false,
        };
        
        // Get parent path and index to delete
        let parent_indices = &indices[..indices.len() - 1];
        let delete_index = indices[indices.len() - 1];
        let parent_path = if parent_indices.is_empty() {
            "root".to_string()
        } else {
            format!("root{}", parent_indices.iter().map(|i| format!("[{}]", i)).collect::<String>())
        };
        
        // Delete from the underlying data structure
        if let Some(root) = &mut self.param_file.root {
            if Self::delete_from_param_value(&mut root.value, parent_indices, delete_index, 0) {
                // Record undo action
                self.push_undo_action(UndoAction::DeleteNode {
                    path: path.to_string(),
                    node: node_to_delete,
                    parent_path,
                    index: delete_index,
                });
                
                // Also delete from the display tree
                Self::delete_from_display_tree(&mut self.param_file.root, parent_indices, delete_index, 0);
                return true;
            }
        }
        
        false
    }
    
    /// Delete from the underlying ParamValue structure
    fn delete_from_param_value(
        value: &mut ParamValue,
        parent_indices: &[usize],
        delete_index: usize,
        depth: usize
    ) -> bool {
        if depth == parent_indices.len() {
            // We're at the parent, delete the child
            match value {
                ParamValue::Struct(ref mut s) => {
                    // Find the hash key at the given index
                    if let Some((hash_to_remove, _)) = s.fields.get_index(delete_index) {
                        let hash_to_remove = *hash_to_remove;
                        s.fields.shift_remove(&hash_to_remove);
                        return true;
                    }
                }
                ParamValue::List(ref mut l) => {
                    if delete_index < l.values.len() {
                        l.values.remove(delete_index);
                        return true;
                    }
                }
                _ => {}
            }
            return false;
        }
        
        // Continue recursing
        let current_index = parent_indices[depth];
        match value {
            ParamValue::Struct(ref mut s) => {
                if let Some((_, field_value)) = s.fields.get_index_mut(current_index) {
                    return Self::delete_from_param_value(field_value, parent_indices, delete_index, depth + 1);
                }
            }
            ParamValue::List(ref mut l) => {
                if current_index < l.values.len() {
                    return Self::delete_from_param_value(&mut l.values[current_index], parent_indices, delete_index, depth + 1);
                }
            }
            _ => {}
        }
        
        false
    }
    
    /// Delete from the display tree
    fn delete_from_display_tree(
        node: &mut Option<ParamNode>,
        parent_indices: &[usize],
        delete_index: usize,
        depth: usize
    ) -> bool {
        if let Some(current_node) = node {
            if depth == parent_indices.len() {
                // We're at the parent, delete the child
                if delete_index < current_node.children.len() {
                    current_node.children.remove(delete_index);
                    return true;
                }
                return false;
            }
            
            // Continue recursing
            let current_index = parent_indices[depth];
            if current_index < current_node.children.len() {
                let mut child_option = Some(std::mem::replace(&mut current_node.children[current_index], ParamNode::new("temp".to_string(), 0, ParamValue::Bool(false))));
                let result = Self::delete_from_display_tree(&mut child_option, parent_indices, delete_index, depth + 1);
                if let Some(updated_child) = child_option {
                    current_node.children[current_index] = updated_child;
                }
                return result;
            }
        }
        false
    }
    
    /// Paste a node into the target path
    fn paste_node_into(&mut self, target_path: &str, node_to_paste: ParamNode) -> bool {
        // Get the target node to determine how to paste
        if let Some(target_node) = self.find_node_by_path(target_path) {
            match &target_node.value {
                // If target is a struct, add source as a new field (regardless of source type)
                ParamValue::Struct(_) => {
                    return self.add_node_with_undo(target_path, node_to_paste);
                }
                // If target is a list, add source as a new item (regardless of source type)
                ParamValue::List(_) => {
                    return self.add_node_with_undo(target_path, node_to_paste);
                }
                _ => {
                    // For other cases (primitive values), replace the value using undo tracking
                    return self.update_node_value_with_undo(target_path, node_to_paste.value);
                }
            }
        }
        
        false
    }
    

    
    /// Add a node to the underlying ParamValue structure
    fn add_to_param_value(
        value: &mut ParamValue,
        indices: &[usize],
        node_to_add: ParamNode,
        depth: usize
    ) -> bool {
        if depth == indices.len() {
            // We're at the target, add the node
            match value {
                ParamValue::Struct(ref mut s) => {
                    // For structs, we need to generate a unique hash if there's a collision
                    let mut hash = node_to_add.hash;
                    let mut counter = 1;
                    while s.fields.contains_key(&hash) {
                        // Generate a new hash by adding a counter
                        hash = node_to_add.hash.wrapping_add(counter);
                        counter += 1;
                        if counter > 1000 {
                            return false; // Prevent infinite loop
                        }
                    }
                    s.fields.insert(hash, node_to_add.value);
                    return true;
                }
                ParamValue::List(ref mut l) => {
                    l.values.push(node_to_add.value);
                    return true;
                }
                _ => return false, // Can only add to structs and lists
            }
        }
        
        // Continue recursing
        let current_index = indices[depth];
        match value {
            ParamValue::Struct(ref mut s) => {
                if let Some((_, field_value)) = s.fields.get_index_mut(current_index) {
                    return Self::add_to_param_value(field_value, indices, node_to_add, depth + 1);
                }
            }
            ParamValue::List(ref mut l) => {
                if current_index < l.values.len() {
                    return Self::add_to_param_value(&mut l.values[current_index], indices, node_to_add, depth + 1);
                }
            }
            _ => {}
        }
        
        false
    }
    
    /// Add a node to the display tree
    #[allow(dead_code)]
    fn add_to_display_tree(
        node: &mut Option<ParamNode>,
        indices: &[usize],
        node_to_add: ParamNode,
        depth: usize
    ) -> bool {
        if let Some(current_node) = node {
            if depth == indices.len() {
                // We're at the target, add the node
                current_node.children.push(node_to_add);
                return true;
            }
            
            // Continue recursing
            let current_index = indices[depth];
            if current_index < current_node.children.len() {
                let mut child_option = Some(std::mem::replace(&mut current_node.children[current_index], ParamNode::new("temp".to_string(), 0, ParamValue::Bool(false))));
                let result = Self::add_to_display_tree(&mut child_option, indices, node_to_add, depth + 1);
                if let Some(updated_child) = child_option {
                    current_node.children[current_index] = updated_child;
                }
                return result;
            }
        }
        false
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
    #[allow(dead_code)]
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
    
    /// Push an action to the undo stack and clear redo stack
    fn push_undo_action(&mut self, action: UndoAction) {
        self.undo_stack.push(action);
        self.redo_stack.clear(); // Clear redo stack when new action is performed
        
        // Limit undo stack size to prevent memory issues
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
    }
    
    /// Perform undo operation
    fn undo(&mut self) -> bool {
        if let Some(action) = self.undo_stack.pop() {
            match action.clone() {
                UndoAction::DeleteNode { path, node, parent_path, index } => {
                    // Restore the deleted node
                    if self.restore_node_at_index(&parent_path, node, index) {
                        self.redo_stack.push(UndoAction::AddNode { path });
                        self.status_message = "Undid delete operation".to_string();
                        self.build_tree_items();
                        return true;
                    }
                }
                UndoAction::AddNode { path } => {
                    // Remove the added node
                    if let Some(node) = self.find_node_by_path(&path).cloned() {
                        if let Some(parent_path) = self.get_parent_path(&path) {
                            if let Some(index) = self.get_node_index_in_parent(&path) {
                                if self.delete_node(&path) {
                                    self.redo_stack.push(UndoAction::DeleteNode { 
                                        path: path.clone(), 
                                        node, 
                                        parent_path, 
                                        index 
                                    });
                                    self.status_message = "Undid add operation".to_string();
                                    self.build_tree_items();
                                    return true;
                                }
                            }
                        }
                    }
                }
                UndoAction::UpdateValue { path, old_value, new_value } => {
                    // Restore the old value
                    if self.param_file.update_node_value(&path, old_value.clone()) {
                        self.redo_stack.push(UndoAction::UpdateValue { 
                            path, 
                            old_value: new_value, 
                            new_value: old_value 
                        });
                        self.status_message = "Undid value change".to_string();
                        self.param_file.rebuild_tree_with_labels();
                        return true;
                    }
                }
                UndoAction::UpdateKey { path, old_name, old_hash, new_name, new_hash } => {
                    // Restore the old key
                    if self.param_file.update_node_key(&path, old_name.clone(), old_hash) {
                        self.redo_stack.push(UndoAction::UpdateKey { 
                            path, 
                            old_name: new_name, 
                            old_hash: new_hash, 
                            new_name: old_name, 
                            new_hash: old_hash 
                        });
                        self.status_message = "Undid key change".to_string();
                        self.param_file.rebuild_tree_with_labels();
                        return true;
                    }
                }
            }
        }
        false
    }
    
    /// Perform redo operation
    fn redo(&mut self) -> bool {
        if let Some(action) = self.redo_stack.pop() {
            match action.clone() {
                UndoAction::DeleteNode { path, node, parent_path, index } => {
                    // Re-delete the node
                    if self.delete_node(&path) {
                        self.undo_stack.push(UndoAction::DeleteNode { path, node, parent_path, index });
                        self.status_message = "Redid delete operation".to_string();
                        self.build_tree_items();
                        return true;
                    }
                }
                UndoAction::AddNode { path: _ } => {
                    // This would require re-adding the node, which is complex
                    // For now, just indicate it's not supported
                    self.status_message = "Redo add operation not yet supported".to_string();
                    return false;
                }
                UndoAction::UpdateValue { path, old_value, new_value } => {
                    // Re-apply the new value
                    if self.param_file.update_node_value(&path, new_value.clone()) {
                        self.undo_stack.push(UndoAction::UpdateValue { 
                            path, 
                            old_value: old_value, 
                            new_value: new_value 
                        });
                        self.status_message = "Redid value change".to_string();
                        self.param_file.rebuild_tree_with_labels();
                        return true;
                    }
                }
                UndoAction::UpdateKey { path, old_name, old_hash, new_name, new_hash } => {
                    // Re-apply the new key
                    if self.param_file.update_node_key(&path, new_name.clone(), new_hash) {
                        self.undo_stack.push(UndoAction::UpdateKey { 
                            path, 
                            old_name, 
                            old_hash, 
                            new_name, 
                            new_hash 
                        });
                        self.status_message = "Redid key change".to_string();
                        self.param_file.rebuild_tree_with_labels();
                        return true;
                    }
                }
            }
        }
        false
    }
    
    /// Get the index of a node within its parent
    fn get_node_index_in_parent(&self, path: &str) -> Option<usize> {
        if let Some(parent_path) = self.get_parent_path(path) {
            if let Some(_parent_node) = self.find_node_by_path(&parent_path) {
                // Extract the index from the path
                if let Some(last_bracket) = path.rfind('[') {
                    if let Some(close_bracket) = path.rfind(']') {
                        let index_str = &path[last_bracket + 1..close_bracket];
                        return index_str.parse::<usize>().ok();
                    }
                }
            }
        }
        None
    }
    
    /// Restore a node at a specific index in its parent
    fn restore_node_at_index(&mut self, parent_path: &str, node: ParamNode, index: usize) -> bool {
        let parent_indices = match self.param_file.parse_node_path(parent_path) {
            Some(indices) => indices,
            None => return false,
        };
        
        // Add to the underlying data structure at the specific index
        if let Some(root) = &mut self.param_file.root {
            if Self::restore_to_param_value(&mut root.value, &parent_indices, node.clone(), index, 0) {
                // Rebuild the display tree to show the restored node
                self.param_file.rebuild_tree_with_labels();
                return true;
            }
        }
        
        false
    }
    
    /// Restore a node to the underlying ParamValue structure at a specific index
    fn restore_to_param_value(
        value: &mut ParamValue,
        indices: &[usize],
        node_to_restore: ParamNode,
        target_index: usize,
        depth: usize
    ) -> bool {
        if depth == indices.len() {
            // We're at the target parent, restore the node at the specific index
            match value {
                ParamValue::Struct(ref mut s) => {
                    // For structs, we need to insert at the correct position
                    // This is complex with IndexMap, so we'll rebuild it
                    let mut new_fields = indexmap::IndexMap::new();
                    let mut current_index = 0;
                    
                    // Copy existing fields, inserting the restored node at the target index
                    for (hash, field_value) in s.fields.iter() {
                        if current_index == target_index {
                            new_fields.insert(node_to_restore.hash, node_to_restore.value.clone());
                            current_index += 1;
                        }
                        new_fields.insert(*hash, field_value.clone());
                        current_index += 1;
                    }
                    
                    // If target index is at the end
                    if target_index >= s.fields.len() {
                        new_fields.insert(node_to_restore.hash, node_to_restore.value);
                    }
                    
                    s.fields = new_fields;
                    return true;
                }
                ParamValue::List(ref mut l) => {
                    if target_index <= l.values.len() {
                        l.values.insert(target_index, node_to_restore.value);
                        return true;
                    }
                }
                _ => return false,
            }
        }
        
        // Continue recursing
        let current_index = indices[depth];
        match value {
            ParamValue::Struct(ref mut s) => {
                if let Some((_, field_value)) = s.fields.get_index_mut(current_index) {
                    return Self::restore_to_param_value(field_value, indices, node_to_restore, target_index, depth + 1);
                }
            }
            ParamValue::List(ref mut l) => {
                if current_index < l.values.len() {
                    return Self::restore_to_param_value(&mut l.values[current_index], indices, node_to_restore, target_index, depth + 1);
                }
            }
            _ => {}
        }
        
        false
    }

    /// Update a node's value with undo tracking
    fn update_node_value_with_undo(&mut self, path: &str, new_value: ParamValue) -> bool {
        // Get the old value for undo
        if let Some(old_value) = self.param_file.get_node_value(path) {
            if self.param_file.update_node_value(path, new_value.clone()) {
                // Record undo action
                self.push_undo_action(UndoAction::UpdateValue {
                    path: path.to_string(),
                    old_value,
                    new_value,
                });
                return true;
            }
        }
        false
    }
    
    /// Update a node's key with undo tracking
    fn update_node_key_with_undo(&mut self, path: &str, new_name: String, new_hash: u64) -> bool {
        // Get the old key for undo
        if let Some(node) = self.find_node_by_path(path) {
            let old_name = node.name.clone();
            let old_hash = node.hash;
            
            if self.param_file.update_node_key(path, new_name.clone(), new_hash) {
                // Record undo action
                self.push_undo_action(UndoAction::UpdateKey {
                    path: path.to_string(),
                    old_name,
                    old_hash,
                    new_name,
                    new_hash,
                });
                return true;
            }
        }
        false
    }
    
    /// Add a node with undo tracking
    fn add_node_with_undo(&mut self, target_path: &str, node_to_add: ParamNode) -> bool {
        let target_indices = match self.param_file.parse_node_path(target_path) {
            Some(indices) => indices,
            None => return false,
        };
        
        // Get the current size to calculate the new index
        let new_index = if let Some(target_node) = self.find_node_by_path(target_path) {
            match &target_node.value {
                ParamValue::Struct(s) => s.fields.len(),
                ParamValue::List(l) => l.values.len(),
                _ => return false,
            }
        } else {
            return false;
        };
        
        // Add to the underlying data structure
        if let Some(root) = &mut self.param_file.root {
            if Self::add_to_param_value(&mut root.value, &target_indices, node_to_add.clone(), 0) {
                // Calculate the path where the node was added
                let added_path = format!("{}[{}]", target_path, new_index);
                
                // Record undo action
                self.push_undo_action(UndoAction::AddNode {
                    path: added_path,
                });
                
                // Rebuild the display tree to show the new node
                self.param_file.rebuild_tree_with_labels();
                return true;
            }
        }
        
        false
    }

    /// Generate a sequential name for a new node to avoid duplicates
    fn generate_sequential_name(&self, parent_path: &str, _base_name: &str) -> String {
        // Get the parent node to check existing children
        if let Some(parent_node) = self.find_node_by_path(parent_path) {
            // Find the highest numeric name among all children
            let mut max_number = 0;
            
            for child in &parent_node.children {
                // Try to parse the child name as a number
                if let Ok(number) = child.name.parse::<u32>() {
                    max_number = max_number.max(number);
                }
                
                // Also try to extract numbers from names like "[18]" 
                let trimmed = child.name.trim_start_matches('[').trim_end_matches(']');
                if let Ok(number) = trimmed.parse::<u32>() {
                    max_number = max_number.max(number);
                }
            }
            
            // Return the next sequential number with brackets
            format!("[{}]", max_number + 1)
        } else {
            // Fallback if we can't find the parent
            "[1]".to_string()
        }
    }
    
    /// Ensure a name is unique by adding _copy suffix if needed
    fn ensure_unique_name(&self, node_path: &str, desired_name: &str) -> String {
        // Get the parent path to check for siblings
        if let Some(parent_path) = self.get_parent_path(node_path) {
            if let Some(parent_node) = self.find_node_by_path(&parent_path) {
                // Check if the desired name conflicts with any sibling (excluding self)
                let current_node_index = self.get_node_index_in_parent(node_path);
                
                let name_conflicts = parent_node.children.iter().enumerate().any(|(i, child)| {
                    // Don't compare with self
                    if let Some(current_index) = current_node_index {
                        if i == current_index {
                            return false;
                        }
                    }
                    child.name == desired_name
                });
                
                if name_conflicts {
                    // Add _# suffix starting from _2
                    let mut copy_counter = 2;
                    loop {
                        let copy_name = format!("{}_{}", desired_name, copy_counter);
                        
                        let copy_exists = parent_node.children.iter().enumerate().any(|(i, child)| {
                            // Don't compare with self
                            if let Some(current_index) = current_node_index {
                                if i == current_index {
                                    return false;
                                }
                            }
                            child.name == copy_name
                        });
                        
                        if !copy_exists {
                            return copy_name;
                        }
                        
                        copy_counter += 1;
                        if copy_counter > 100 {
                            break; // Prevent infinite loop
                        }
                    }
                }
            }
        }
        
        // No conflict or couldn't check, return original name
        desired_name.to_string()
    }
    
    /// Generate a name for pasting, preserving original names when possible
    fn generate_paste_name(&self, parent_path: &str, original_name: &str) -> String {
        // If the original name is not a number or bracketed number, try to preserve it
        let is_numeric = original_name.parse::<u32>().is_ok();
        let is_bracketed_numeric = {
            let trimmed = original_name.trim_start_matches('[').trim_end_matches(']');
            original_name.starts_with('[') && original_name.ends_with(']') && trimmed.parse::<u32>().is_ok()
        };
        
        if !is_numeric && !is_bracketed_numeric {
            // Check if this name already exists in the parent
            if let Some(parent_node) = self.find_node_by_path(parent_path) {
                let name_exists = parent_node.children.iter().any(|child| child.name == original_name);
                if !name_exists {
                    // Original name doesn't exist, we can use it as-is
                    return original_name.to_string();
                }
                
                // If it exists, try adding _# suffix for text names starting from _2
                let mut copy_counter = 2;
                loop {
                    let copy_name = format!("{}_{}", original_name, copy_counter);
                    
                    let copy_exists = parent_node.children.iter().any(|child| child.name == copy_name);
                    if !copy_exists {
                        return copy_name;
                    }
                    
                    copy_counter += 1;
                    if copy_counter > 100 {
                        break; // Prevent infinite loop
                    }
                }
            }
        }
        
        // For numeric names or when name conflicts exist, generate sequential name
        self.generate_sequential_name(parent_path, original_name)
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
        let _ = std::fs::write(&config_path, path);
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
        // Try to handle clipboard operations using egui's events
        ctx.input_mut(|i| {
            // Check for copy/paste events that egui might have processed
            if !i.events.is_empty() {
                for event in &i.events {
                    match event {
                        egui::Event::Copy => {
                            if let Some(selected_path) = &self.selected_node {
                                self.clipboard = Some(selected_path.clone());
                                self.clipboard_data = self.find_node_by_path(selected_path).cloned();
                                self.cut_mode = false;
                                let has_data = self.clipboard_data.is_some();
                                self.status_message = format!("Copied node via event: {} (data: {})", selected_path, has_data);
                            } else {
                                self.status_message = "Copy event: No node selected".to_string();
                            }
                            return;
                        }
                        egui::Event::Paste(_text) => {
                            // Handle paste using our internal clipboard
                            if let (Some(clipboard_data), Some(selected_path)) = (self.clipboard_data.clone(), self.selected_node.clone()) {
                                if self.paste_node_into(&selected_path, clipboard_data) {
                                    let action = if self.cut_mode { "Moved" } else { "Pasted" };
                                    self.status_message = format!("{} node into {} via paste event", action, selected_path);
                                    
                                    // For cut operations, clear the clipboard since it's now moved
                                    if self.cut_mode {
                                        self.clipboard = None;
                                        self.clipboard_data = None;
                                        self.cut_mode = false;
                                    }
                                    
                                    // Rebuild tree items to show changes
                                    self.build_tree_items();
                                } else {
                                    self.status_message = format!("Failed to paste into {} via paste event", selected_path);
                                }
                            } else {
                                self.status_message = "Paste event: Nothing to paste".to_string();
                            }
                            return;
                        }
                        _ => {}
                    }
                }
            }
            

            
            // Try alternative shortcut detection using egui's shortcut system
            if !self.editing_value.is_some() {
                // Try using egui's shortcut detection
                if i.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::V)) ||
                   i.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::SHIFT, egui::Key::V)) {
                    // Handle paste logic here
                    if let (Some(clipboard_data), Some(selected_path)) = (self.clipboard_data.clone(), self.selected_node.clone()) {
                        if self.paste_node_into(&selected_path, clipboard_data.clone()) {
                            let action = if self.cut_mode { "Moved" } else { "Pasted" };
                            let paste_type = match (&clipboard_data.value, self.find_node_by_path(&selected_path).map(|n| &n.value)) {
                                (ParamValue::Struct(_), Some(ParamValue::Struct(_))) => "fields",
                                (ParamValue::List(_), Some(ParamValue::List(_))) => "items",
                                _ => "node"
                            };
                            self.status_message = format!("{} {} into {} with Ctrl+V (shortcut)", action, paste_type, selected_path);
                            
                            if self.cut_mode {
                                self.clipboard = None;
                                self.clipboard_data = None;
                                self.cut_mode = false;
                            }
                            self.build_tree_items();
                        } else {
                            self.status_message = format!("Failed to paste into {} with Ctrl+V (shortcut)", selected_path);
                        }
                    } else {
                        self.status_message = "Ctrl+V (shortcut): Nothing to paste".to_string();
                    }
                }
                
                if i.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::SHIFT, egui::Key::Insert)) ||
                   i.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::ALT, egui::Key::Insert)) {
                    // Handle paste logic here
                    if let (Some(clipboard_data), Some(selected_path)) = (self.clipboard_data.clone(), self.selected_node.clone()) {
                        if self.paste_node_into(&selected_path, clipboard_data.clone()) {
                            let action = if self.cut_mode { "Moved" } else { "Pasted" };
                            let paste_type = match (&clipboard_data.value, self.find_node_by_path(&selected_path).map(|n| &n.value)) {
                                (ParamValue::Struct(_), Some(ParamValue::Struct(_))) => "fields",
                                (ParamValue::List(_), Some(ParamValue::List(_))) => "items",
                                _ => "node"
                            };
                            self.status_message = format!("{} {} into {} with Shift+Insert (shortcut)", action, paste_type, selected_path);
                            
                            if self.cut_mode {
                                self.clipboard = None;
                                self.clipboard_data = None;
                                self.cut_mode = false;
                            }
                            self.build_tree_items();
                        } else {
                            self.status_message = format!("Failed to paste into {} with Shift+Insert (shortcut)", selected_path);
                        }
                    } else {
                        self.status_message = "Shift+Insert (shortcut): Nothing to paste".to_string();
                    }
                }
            }
            
            // Only handle shortcuts if no text editing is active
            if !self.editing_value.is_some() {
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
                    if let Some(selected_path) = self.selected_node.clone() {
                        if self.delete_node(&selected_path) {
                            self.status_message = format!("Deleted node: {}", selected_path);
                            // Clear selection since the node no longer exists
                            self.selected_node = None;
                            self.selected_index = None;
                            // Rebuild tree items
                            self.build_tree_items();
                        } else {
                            self.status_message = format!("Failed to delete node: {}", selected_path);
                        }
                    }
                }
                
                // CTRL + C - Copy the node (try multiple approaches)
                if (ctrl && i.key_pressed(egui::Key::C)) || 
                   (ctrl && i.modifiers.shift && i.key_pressed(egui::Key::C)) ||
                   (ctrl && i.key_pressed(egui::Key::Insert)) {
                    if let Some(selected_path) = &self.selected_node {
                        self.clipboard = Some(selected_path.clone());
                        let node_data = self.find_node_by_path(selected_path).cloned();
                        

                        
                        self.clipboard_data = node_data;
                        self.cut_mode = false;
                        let shortcut = if i.key_pressed(egui::Key::Insert) { "Ctrl+Insert" } 
                                      else if i.modifiers.shift { "Ctrl+Shift+C" } 
                                      else { "Ctrl+C" };
                        self.status_message = format!("Copied node with {}: {}", shortcut, selected_path);
                    } else {
                        self.status_message = "No node selected to copy".to_string();
                    }
                }
                
                // CTRL + X - Cut the node
                if ctrl && i.key_pressed(egui::Key::X) {
                    if let Some(selected_path) = self.selected_node.clone() {
                        // First copy the node data
                        if let Some(node_data) = self.find_node_by_path(&selected_path).cloned() {
                        self.clipboard = Some(selected_path.clone());
                            self.clipboard_data = Some(node_data);
                            self.cut_mode = true;
                            
                            // Then delete the node from its current location
                            if self.delete_node(&selected_path) {
                                self.status_message = format!("Cut node: {}", selected_path);
                                // Clear selection since the node no longer exists
                                self.selected_node = None;
                                self.selected_index = None;
                                // Rebuild tree items
                                self.build_tree_items();
                            } else {
                                self.status_message = format!("Failed to cut node: {}", selected_path);
                                // Clear clipboard if cut failed
                                self.clipboard = None;
                                self.clipboard_data = None;
                                self.cut_mode = false;
                            }
                        } else {
                            self.status_message = format!("Could not find node to cut: {}", selected_path);
                        }
                    }
                }
                
                // Note: Ctrl+V paste is handled by the egui shortcut system above
                // Only handle the alternative paste shortcuts here that egui might not catch
                if (i.modifiers.shift && i.key_pressed(egui::Key::V) && !ctrl) ||  // Shift+V (when Ctrl+V is detected as Shift+V)
                   (i.modifiers.alt && i.key_pressed(egui::Key::Insert)) {  // Alt+Insert (when Shift+Insert is detected as Alt+Insert)
                    let shortcut = if i.modifiers.alt && i.key_pressed(egui::Key::Insert) { "Shift+Insert (detected as Alt+Insert)" }
                                  else { "Ctrl+V (detected as Shift+V)" };
                    
                    if let (Some(clipboard_data), Some(selected_path)) = (self.clipboard_data.clone(), self.selected_node.clone()) {
                        if self.paste_node_into(&selected_path, clipboard_data.clone()) {
                            let action = if self.cut_mode { "Moved" } else { "Pasted" };
                            let paste_type = match self.find_node_by_path(&selected_path).map(|n| &n.value) {
                                Some(ParamValue::Struct(_)) => "node into struct",
                                Some(ParamValue::List(_)) => "node into list",
                                _ => "node"
                            };
                            self.status_message = format!("{} {} {} with {}", action, paste_type, selected_path, shortcut);
                            
                            // For cut operations, clear the clipboard since it's now moved
                            if self.cut_mode {
                                self.clipboard = None;
                                self.clipboard_data = None;
                                self.cut_mode = false;
                            }
                            
                            // Rebuild tree items to show changes
                            self.build_tree_items();
                        } else {
                            self.status_message = format!("Failed to paste into {} with {}", selected_path, shortcut);
                        }
                    } else {
                        let has_clipboard = self.clipboard.is_some();
                        let has_data = self.clipboard_data.is_some();
                        let has_selection = self.selected_node.is_some();
                        self.status_message = format!("{}: Nothing to paste (clipboard: {}, data: {}, selection: {})", 
                            shortcut, has_clipboard, has_data, has_selection);
                    }
                }
                
                // CTRL + P - Paste the copied node into the parent
                if ctrl && i.key_pressed(egui::Key::P) {
                    if let (Some(clipboard_data), Some(selected_path)) = (self.clipboard_data.clone(), self.selected_node.clone()) {
                        if let Some(parent_path) = self.get_parent_path(&selected_path) {
                            // Generate a new name for the pasted node, preserving original names when possible
                            let mut new_clipboard_data = clipboard_data.clone();
                            let original_name = &clipboard_data.name;
                            let generated_name = self.generate_paste_name(&parent_path, original_name);
                            new_clipboard_data.name = generated_name.clone();
                            new_clipboard_data.hash = self.param_file.hash_labels.add_label_and_save(&new_clipboard_data.name, self.param_labels_path.as_deref());
                            

                            
                            if self.paste_node_into(&parent_path, new_clipboard_data) {
                                let action = if self.cut_mode { "Moved" } else { "Pasted" };
                                self.status_message = format!("{} node into parent of {}", action, selected_path);
                                
                                // For cut operations, clear the clipboard since it's now moved
                                if self.cut_mode {
                                    self.clipboard = None;
                                    self.clipboard_data = None;
                                    self.cut_mode = false;
                                }
                                
                                // Rebuild tree items to show changes
                                self.build_tree_items();
                            } else {
                                self.status_message = format!("Failed to paste into parent of {}", selected_path);
                            }
                        } else {
                            self.status_message = "Root node has no parent".to_string();
                        }
                    } else {
                        self.status_message = "Nothing to paste into parent".to_string();
                    }
                }
                
                // CTRL + D - Duplicate param on the same level
                if ctrl && i.key_pressed(egui::Key::D) {
                    if let Some(selected_path) = self.selected_node.clone() {
                        if let Some(node_to_duplicate) = self.find_node_by_path(&selected_path).cloned() {
                            if let Some(parent_path) = self.get_parent_path(&selected_path) {
                                // Generate a new name for the duplicated node
                                let mut new_node = node_to_duplicate.clone();
                                new_node.name = self.generate_sequential_name(&parent_path, &node_to_duplicate.name);
                                new_node.hash = self.param_file.hash_labels.add_label_and_save(&new_node.name, self.param_labels_path.as_deref());
                                
                                if self.paste_node_into(&parent_path, new_node) {
                                    self.status_message = format!("Duplicated node: {}", selected_path);
                                    // Rebuild tree items to show changes
                                    self.build_tree_items();
                                } else {
                                    self.status_message = format!("Failed to duplicate node: {}", selected_path);
                                }
                            } else {
                                self.status_message = "Cannot duplicate root node".to_string();
                            }
                        } else {
                            self.status_message = format!("Could not find node to duplicate: {}", selected_path);
                        }
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
                
                // CTRL + Z - Undo
                if ctrl && i.key_pressed(egui::Key::Z) {
                    if self.undo() {
                        // Undo was successful, status message is set by undo()
                    } else {
                        self.status_message = "Nothing to undo".to_string();
                    }
                }
                
                // CTRL + Y - Redo
                if ctrl && i.key_pressed(egui::Key::Y) {
                    if self.redo() {
                        // Redo was successful, status message is set by redo()
                    } else {
                        self.status_message = "Nothing to redo".to_string();
                    }
                }
                
                // F2 - Rename selected node
                if i.key_pressed(egui::Key::F2) {
                    if let Some(selected_path) = &self.selected_node {
                        if let Some(node) = self.find_node_by_path(selected_path) {
                            let name_edit_path = format!("{}_name", selected_path);
                            self.editing_value = Some((name_edit_path, node.name.clone()));
                            self.status_message = "Press Enter to confirm rename, Escape to cancel".to_string();
                        }
                    }
                }
                
                // F1 key removed - shortcuts are now always visible
            }
        });
    }


}

impl eframe::App for PrcEditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle keyboard shortcuts
        self.handle_keyboard_shortcuts(ctx);
        
        // Status bar at bottom using bottom panel - create this FIRST so main content knows about it
        egui::TopBottomPanel::bottom("status_panel")
            .resizable(false)
            .min_height(25.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Status:");
                    ui.label(&self.status_message);
                    
                    // Show paste buttons for testing
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        
                        // Add paste button for testing
                        if let Some(_) = &self.clipboard_data {
                            if ui.button("Paste").clicked() {
                                if let Some(selected_path) = self.selected_node.clone() {
                                    if let Some(clipboard_data) = self.clipboard_data.clone() {
                                        if self.paste_node_into(&selected_path, clipboard_data.clone()) {
                                            let action = if self.cut_mode { "Moved" } else { "Pasted" };
                                            let paste_type = match self.find_node_by_path(&selected_path).map(|n| &n.value) {
                                                Some(ParamValue::Struct(_)) => "node into struct",
                                                Some(ParamValue::List(_)) => "node into list",
                                                _ => "node"
                                            };
                                            self.status_message = format!("{} {} {} via button", action, paste_type, selected_path);
                                            
                                            if self.cut_mode {
                                                self.clipboard = None;
                                                self.clipboard_data = None;
                                                self.cut_mode = false;
                                            }
                                            self.build_tree_items();
                                        } else {
                                            self.status_message = format!("Failed to paste into {} via button", selected_path);
                                        }
                                    }
                                } else {
                                    self.status_message = "No node selected for paste".to_string();
                                }
                            }
                        }
                        
                        // Show clipboard status
                        if let Some(clipboard_path) = &self.clipboard {
                            let mode = if self.cut_mode { "Cut" } else { "Copy" };
                            let has_data = self.clipboard_data.is_some();
                            ui.label(&format!("Clipboard: {} {} (data: {})", mode, clipboard_path, has_data));
                        }
                        
                        // Show undo/redo stack info
                        ui.label(&format!("Undo: {} | Redo: {}", self.undo_stack.len(), self.redo_stack.len()));
                        
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
        
        // Main content area - now it knows about the status bar space
        egui::CentralPanel::default().show(ctx, |ui| {
            // Menu bar
            self.show_menu_bar(ctx, ui);
            
            ui.separator();
            
            // Main content area
            self.show_main_content(ui);
        });
        
        // Show label editor window if open
        self.show_label_editor_window(ctx);
    }
} 