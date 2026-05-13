use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, CheckButton, Label, ListBox,
    ListBoxRow, Notebook, Orientation, PolicyType, ScrolledWindow, SearchEntry, SelectionMode,
    Separator,
};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

mod nix_ops;
use nix_ops::Package;

const APP_ID: &str = "com.spectreOS.Updater";

struct State {
    installed: Vec<String>,
    installed_versions: HashMap<String, String>,
    available_versions: HashMap<String, String>,
    staged_add: HashMap<String, Package>,
    staged_remove: HashSet<String>,
}

impl State {
    fn needs_apply(&self) -> bool {
        !self.staged_add.is_empty() || !self.staged_remove.is_empty()
    }

    fn is_managed(&self, pname: &str) -> bool {
        self.installed.contains(&pname.to_string())
            || self.installed.contains(&format!("unstable.{}", pname))
    }

    fn final_packages(&self) -> Vec<String> {
        let mut pkgs: Vec<String> = self.installed
            .iter()
            .filter(|p| !self.staged_remove.contains(*p))
            .cloned()
            .collect();
        for (pname, pkg) in &self.staged_add {
            let install_name = if pkg.is_unstable {
                format!("unstable.{}", pname)
            } else {
                pname.clone()
            };
            if !pkgs.contains(&install_name) {
                pkgs.push(install_name);
            }
        }
        pkgs.sort();
        pkgs
    }

    fn final_versions(&self) -> HashMap<String, String> {
        let mut versions = self.installed_versions.clone();
        for p in &self.staged_remove {
            let pname = p.split('.').last().unwrap_or(p.as_str());
            versions.remove(pname);
        }
        for (pname, pkg) in &self.staged_add {
            if !pkg.version.is_empty() {
                versions.insert(pname.clone(), pkg.version.clone());
            }
        }
        versions
    }

    fn has_update(&self, pname: &str) -> bool {
        match (self.installed_versions.get(pname), self.available_versions.get(pname)) {
            (Some(installed), Some(available)) => installed != available,
            (None, Some(_)) => true,
            _ => false,
        }
    }

    fn any_updates(&self) -> bool {
        self.installed.iter().any(|p| {
            let pname = p.split('.').last().unwrap_or(p.as_str());
            self.has_update(pname)
        })
    }
}

fn main() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &Application) {
    let state: Rc<RefCell<State>> = Rc::new(RefCell::new(State {
        installed: nix_ops::read_installed_packages(),
        installed_versions: nix_ops::read_installed_versions(),
        available_versions: HashMap::new(),
        staged_add: HashMap::new(),
        staged_remove: HashSet::new(),
    }));

    let css = gtk4::CssProvider::new();
    css.load_from_string(
        ".update-badge { color: #3584e4; font-size: 0.8em; \
         border: 1px solid #3584e4; border-radius: 4px; padding: 0 6px; margin-start: 6px; } \
         .install-badge { color: #26a269; font-weight: bold; min-width: 14px; } \
         .remove-badge  { color: #c01c28; font-weight: bold; min-width: 14px; }",
    );
    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().expect("no display"),
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let window = ApplicationWindow::builder()
        .application(app)
        .title("SpectreOS Package Manager")
        .default_width(820)
        .default_height(740)
        .build();

    let root = GtkBox::new(Orientation::Vertical, 10);
    root.set_margin_top(16);
    root.set_margin_bottom(16);
    root.set_margin_start(16);
    root.set_margin_end(16);

    let title = Label::new(Some("SpectreOS Package Manager"));
    title.add_css_class("title-2");
    title.set_halign(Align::Start);
    root.append(&title);

    // ── Shared persistent widgets ────────────────────────────────────────
    let staged_list = ListBox::new();
    staged_list.set_selection_mode(SelectionMode::None);
    let sp = Label::new(Some("No changes staged"));
    staged_list.set_placeholder(Some(&sp));

    let status_label = Label::new(Some(""));
    status_label.set_halign(Align::Start);
    status_label.set_wrap(true);

    let apply_btn = Button::with_label("Install");
    apply_btn.add_css_class("suggested-action");
    apply_btn.set_sensitive(false);

    let update_all_btn = Button::with_label("Update All");
    update_all_btn.add_css_class("suggested-action");
    update_all_btn.set_sensitive(false);

    // ── Tab 1: Installed ─────────────────────────────────────────────────
    let installed_list = ListBox::new();
    installed_list.set_selection_mode(SelectionMode::None);
    let ip = Label::new(Some("No packages installed yet"));
    installed_list.set_placeholder(Some(&ip));
    let installed_scroll = ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(PolicyType::Never)
        .child(&installed_list)
        .build();

    let installed_tab = GtkBox::new(Orientation::Vertical, 0);
    installed_tab.append(&installed_scroll);

    // ── Tab 2: Browse ────────────────────────────────────────────────────
    let search = SearchEntry::new();
    search.set_hexpand(true);
    search.set_placeholder_text(Some("Search nixpkgs — e.g. helix, micro, obsidian"));

    let unstable_check = CheckButton::with_label("Include unstable");
    unstable_check.set_valign(Align::Center);

    let search_row = GtkBox::new(Orientation::Horizontal, 8);
    search_row.set_margin_top(8);
    search_row.set_margin_bottom(4);
    search_row.append(&search);
    search_row.append(&unstable_check);

    let results_list = ListBox::new();
    results_list.set_selection_mode(SelectionMode::None);
    let rp = Label::new(Some("Search for a package above"));
    results_list.set_placeholder(Some(&rp));
    let results_scroll = ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(PolicyType::Never)
        .child(&results_list)
        .build();

    let browse_tab = GtkBox::new(Orientation::Vertical, 4);
    browse_tab.append(&search_row);
    browse_tab.append(&results_scroll);

    // ── Tab 3: System ────────────────────────────────────────────────────
    let system_tab = build_system_tab(&status_label);
    let system_scroll = ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(PolicyType::Never)
        .min_content_height(150)
        .child(&system_tab)
        .build();

    // ── Notebook ─────────────────────────────────────────────────────────
    let notebook = Notebook::new();
    notebook.set_vexpand(true);
    notebook.append_page(&installed_tab, Some(&Label::new(Some("Installed Apps"))));
    notebook.append_page(&browse_tab, Some(&Label::new(Some("Search Apps"))));
    notebook.append_page(&system_scroll, Some(&Label::new(Some("System Updates"))));
    root.append(&notebook);

    // ── Staging area (persistent below notebook) ─────────────────────────
    root.append(&Separator::new(Orientation::Horizontal));
    let staged_label = Label::new(Some("Staged Changes"));
    staged_label.set_halign(Align::Start);
    root.append(&staged_label);

    let staged_scroll = ScrolledWindow::builder()
        .min_content_height(80)
        .vexpand(false)
        .hscrollbar_policy(PolicyType::Never)
        .child(&staged_list)
        .build();
    root.append(&staged_scroll);

    root.append(&status_label);

    // ── Bottom bar ───────────────────────────────────────────────────────
    let bottom_bar = GtkBox::new(Orientation::Horizontal, 8);
    let quit_btn = Button::with_label("Quit");
    bottom_bar.append(&quit_btn);
    let spacer = Label::new(None);
    spacer.set_hexpand(true);
    bottom_bar.append(&spacer);
    bottom_bar.append(&update_all_btn);
    bottom_bar.append(&apply_btn);
    root.append(&bottom_bar);

    window.set_child(Some(&root));

    rebuild_installed_list(&installed_list, &staged_list, &state, &apply_btn, &update_all_btn, &status_label);

    window.present();

    // ── Quit ─────────────────────────────────────────────────────────────
    {
        let window = window.clone();
        quit_btn.connect_clicked(move |_| window.close());
    }

    // ── Search (Tab 2) ────────────────────────────────────────────────────
    let trigger_search = {
        let results_list = results_list.clone();
        let staged_list = staged_list.clone();
        let state = state.clone();
        let apply_btn = apply_btn.clone();
        let search = search.clone();
        let unstable_check = unstable_check.clone();

        Rc::new(move || {
            let query = search.text().to_string();
            while let Some(child) = results_list.first_child() {
                results_list.remove(&child);
            }
            if query.trim().is_empty() { return; }

            let loading = ListBoxRow::new();
            loading.set_child(Some(&Label::new(Some("Searching…"))));
            results_list.append(&loading);

            let include_unstable = unstable_check.is_active();
            let (sender, receiver) = async_channel::bounded::<Vec<Package>>(1);
            std::thread::spawn(move || {
                let _ = sender.send_blocking(nix_ops::search(&query, include_unstable));
            });

            let results_list2 = results_list.clone();
            let staged_list2 = staged_list.clone();
            let state2 = state.clone();
            let apply_btn2 = apply_btn.clone();

            glib::MainContext::default().spawn_local(async move {
                if let Ok(packages) = receiver.recv().await {
                    while let Some(child) = results_list2.first_child() {
                        results_list2.remove(&child);
                    }
                    for pkg in packages {
                        let row = make_result_row(
                            pkg, state2.clone(), staged_list2.clone(), apply_btn2.clone(),
                        );
                        results_list2.append(&row);
                    }
                }
            });
        })
    };

    {
        let trigger = trigger_search.clone();
        search.connect_search_changed(move |_| trigger());
    }
    {
        let trigger = trigger_search.clone();
        unstable_check.connect_toggled(move |_| trigger());
    }

    // ── Version check on launch ───────────────────────────────────────────
    {
        let state = state.clone();
        let status_label = status_label.clone();
        let installed_list = installed_list.clone();
        let staged_list = staged_list.clone();
        let update_all_btn = update_all_btn.clone();
        let apply_btn = apply_btn.clone();

        let managed: Vec<(String, bool)> = state.borrow().installed.iter()
            .map(|p| {
                let is_unstable = p.starts_with("unstable.");
                let pname = if is_unstable { p["unstable.".len()..].to_string() } else { p.clone() };
                (pname, is_unstable)
            })
            .collect();

        if !managed.is_empty() {
            status_label.set_text("Checking for updates…");

            let (sender, receiver) = async_channel::bounded::<HashMap<String, String>>(1);
            std::thread::spawn(move || {
                let _ = sender.send_blocking(nix_ops::fetch_available_versions(&managed));
            });

            glib::MainContext::default().spawn_local(async move {
                if let Ok(available) = receiver.recv().await {
                    state.borrow_mut().available_versions = available;

                    let has_updates = state.borrow().any_updates();
                    if has_updates {
                        status_label.set_text("Updates available");
                        update_all_btn.set_sensitive(true);
                    } else if !state.borrow().installed.is_empty() {
                        status_label.set_text("All packages up to date");
                    } else {
                        status_label.set_text("");
                    }

                    rebuild_installed_list(&installed_list, &staged_list, &state, &apply_btn, &update_all_btn, &status_label);
                }
            });
        }
    }

    // ── Apply staged changes ──────────────────────────────────────────────
    {
        let state = state.clone();
        let status_label = status_label.clone();
        let apply_btn_ref = apply_btn.clone();
        let update_all_btn_ref = update_all_btn.clone();
        let staged_list = staged_list.clone();
        let installed_list = installed_list.clone();
        let search = search.clone();
        let results_list = results_list.clone();

        apply_btn.connect_clicked(move |btn| {
            if !state.borrow().needs_apply() { return; }
            let final_packages = state.borrow().final_packages();
            let final_versions = state.borrow().final_versions();

            btn.set_sensitive(false);
            btn.set_label("Applying…");
            status_label.set_text("Applying changes…");

            let (sender, receiver) = async_channel::bounded::<Result<(), String>>(1);
            let pkgs = final_packages.clone();
            let vers = final_versions.clone();
            std::thread::spawn(move || {
                let result = nix_ops::write_extra_packages(&pkgs, &vers)
                    .map_err(|e| e.to_string())
                    .and_then(|_| nix_ops::run_home_manager());
                let _ = sender.send_blocking(result);
            });

            let btn2 = apply_btn_ref.clone();
            let update_all_btn2 = update_all_btn_ref.clone();
            let status2 = status_label.clone();
            let state2 = state.clone();
            let staged2 = staged_list.clone();
            let installed2 = installed_list.clone();
            let search2 = search.clone();
            let results2 = results_list.clone();

            glib::MainContext::default().spawn_local(async move {
                if let Ok(result) = receiver.recv().await {
                    match result {
                        Ok(()) => {
                            status2.set_text("Done! Changes applied.");
                            {
                                let mut s = state2.borrow_mut();
                                s.installed = final_packages.clone();
                                s.installed_versions = final_versions.clone();
                                s.staged_add.clear();
                                s.staged_remove.clear();
                            }
                            while let Some(child) = staged2.first_child() {
                                staged2.remove(&child);
                            }
                            rebuild_installed_list(&installed2, &staged2, &state2, &btn2, &update_all_btn2, &status2);
                            update_apply_btn(&btn2, &state2);
                            update_all_btn2.set_sensitive(state2.borrow().any_updates());
                            search2.set_text("");
                            while let Some(child) = results2.first_child() {
                                results2.remove(&child);
                            }
                        }
                        Err(ref e) => {
                            status2.set_text(&format!("Error: {}", e));
                            update_apply_btn(&btn2, &state2);
                        }
                    }
                }
            });
        });
    }

    // ── Update All ────────────────────────────────────────────────────────
    {
        let state = state.clone();
        let status_label = status_label.clone();
        let update_all_btn_ref = update_all_btn.clone();
        let installed_list = installed_list.clone();
        let staged_list = staged_list.clone();
        let apply_btn = apply_btn.clone();

        update_all_btn.connect_clicked(move |btn| {
            btn.set_sensitive(false);
            btn.set_label("Updating…");
            status_label.set_text("Updating packages…");

            let (sender, receiver) = async_channel::bounded::<Result<(), String>>(1);
            std::thread::spawn(move || {
                let _ = sender.send_blocking(nix_ops::run_home_manager());
            });

            let btn2 = update_all_btn_ref.clone();
            let status2 = status_label.clone();
            let state2 = state.clone();
            let installed2 = installed_list.clone();
            let staged2 = staged_list.clone();
            let apply_btn2 = apply_btn.clone();

            glib::MainContext::default().spawn_local(async move {
                if let Ok(result) = receiver.recv().await {
                    btn2.set_label("Update All");
                    match result {
                        Ok(()) => {
                            let pkgs = {
                                let mut s = state2.borrow_mut();
                                for (pname, ver) in s.available_versions.clone() {
                                    s.installed_versions.insert(pname, ver);
                                }
                                s.available_versions.clear();
                                s.installed.clone()
                            };
                            let versions = state2.borrow().installed_versions.clone();
                            let _ = nix_ops::write_extra_packages(&pkgs, &versions);
                            status2.set_text("All packages updated.");
                            rebuild_installed_list(&installed2, &staged2, &state2, &apply_btn2, &btn2, &status2);
                            btn2.set_sensitive(false);
                        }
                        Err(ref e) => {
                            status2.set_text(&format!("Update failed: {}", e));
                            btn2.set_sensitive(true);
                        }
                    }
                }
            });
        });
    }
}

fn build_system_tab(status_label: &Label) -> GtkBox {
    let vbox = GtkBox::new(Orientation::Vertical, 12);
    vbox.set_margin_top(16);
    vbox.set_margin_bottom(16);
    vbox.set_margin_start(16);
    vbox.set_margin_end(16);
    vbox.set_valign(Align::Start);

    let current_ver = nix_ops::nixos_version();
    let next_ver = nix_ops::next_nixos_version(&current_ver).unwrap_or_default();

    let version_row = GtkBox::new(Orientation::Horizontal, 8);
    let ver_title = Label::new(Some("Current NixOS version:"));
    ver_title.set_halign(Align::Start);
    let ver_value = Label::new(Some(if current_ver.is_empty() { "unknown" } else { &current_ver }));
    ver_value.add_css_class("title-4");
    version_row.append(&ver_title);
    version_row.append(&ver_value);
    vbox.append(&version_row);

    if next_ver.is_empty() {
        let msg = Label::new(Some("Could not determine the next release version."));
        msg.add_css_class("dim-label");
        msg.set_halign(Align::Start);
        vbox.append(&msg);
        return vbox;
    }

    let upgrade_status = Label::new(Some(""));
    upgrade_status.set_halign(Align::Start);

    let upgrade_btn = Button::with_label(&format!("Upgrade to {}", next_ver));
    upgrade_btn.add_css_class("suggested-action");
    upgrade_btn.set_sensitive(false);
    upgrade_btn.set_visible(false);
    upgrade_btn.set_halign(Align::Start);

    let check_btn = Button::with_label("Check for Upgrade");
    check_btn.set_halign(Align::Start);

    // Check for upgrade
    {
        let upgrade_status = upgrade_status.clone();
        let upgrade_btn = upgrade_btn.clone();
        let check_btn_ref = check_btn.clone();
        let next_ver = next_ver.clone();

        check_btn.connect_clicked(move |btn| {
            btn.set_sensitive(false);
            btn.set_label("Checking…");
            upgrade_status.set_text("Checking availability…");

            let (sender, receiver) = async_channel::bounded::<bool>(1);
            let ver = next_ver.clone();
            std::thread::spawn(move || {
                let _ = sender.send_blocking(nix_ops::check_upgrade_available(&ver));
            });

            let btn2 = check_btn_ref.clone();
            let upgrade_status2 = upgrade_status.clone();
            let upgrade_btn2 = upgrade_btn.clone();
            let next_ver2 = next_ver.clone();

            glib::MainContext::default().spawn_local(async move {
                if let Ok(available) = receiver.recv().await {
                    btn2.set_label("Check for Upgrade");
                    btn2.set_sensitive(true);
                    if available {
                        upgrade_status2.set_text(&format!("NixOS {} is available!", next_ver2));
                        upgrade_btn2.set_visible(true);
                        upgrade_btn2.set_sensitive(true);
                    } else {
                        upgrade_status2.set_text(&format!("NixOS {} is not yet available.", next_ver2));
                    }
                }
            });
        });
    }

    // Run upgrade
    {
        let upgrade_btn_ref = upgrade_btn.clone();
        let upgrade_status = upgrade_status.clone();
        let check_btn = check_btn.clone();
        let status_label = status_label.clone();
        let next_ver = next_ver.clone();

        upgrade_btn.connect_clicked(move |btn| {
            btn.set_sensitive(false);
            btn.set_label("Upgrading…");
            check_btn.set_sensitive(false);
            upgrade_status.set_text("Running system upgrade — this will take a while…");
            status_label.set_text("System upgrade in progress…");

            let (sender, receiver) = async_channel::bounded::<Result<(), String>>(1);
            let ver = next_ver.clone();
            std::thread::spawn(move || {
                let _ = sender.send_blocking(nix_ops::run_system_upgrade(&ver));
            });

            let btn2 = upgrade_btn_ref.clone();
            let upgrade_status2 = upgrade_status.clone();
            let check_btn2 = check_btn.clone();
            let status_label2 = status_label.clone();
            let next_ver2 = next_ver.clone();

            glib::MainContext::default().spawn_local(async move {
                if let Ok(result) = receiver.recv().await {
                    check_btn2.set_sensitive(true);
                    match result {
                        Ok(()) => {
                            btn2.set_visible(false);
                            upgrade_status2.set_text(&format!(
                                "Upgrade to {} complete! Reboot to finish.", next_ver2
                            ));
                            status_label2.set_text(&format!("System upgraded to {}", next_ver2));
                        }
                        Err(ref e) => {
                            btn2.set_sensitive(true);
                            btn2.set_label(&format!("Upgrade to {}", next_ver2));
                            upgrade_status2.set_text(&format!("Upgrade failed: {}", e));
                            status_label2.set_text("Upgrade failed.");
                        }
                    }
                }
            });
        });
    }

    vbox.append(&check_btn);
    vbox.append(&upgrade_status);
    vbox.append(&upgrade_btn);

    let note = Label::new(Some("A reboot is required after upgrading to complete the transition."));
    note.add_css_class("dim-label");
    note.set_halign(Align::Start);
    note.set_wrap(true);
    vbox.append(&note);

    // ── Apply Updates ─────────────────────────────────────────────────────
    let sep1 = gtk4::Separator::new(Orientation::Horizontal);
    sep1.set_margin_top(8);
    sep1.set_margin_bottom(8);
    vbox.append(&sep1);

    let rebuild_title = Label::new(Some("System Updates"));
    rebuild_title.add_css_class("title-4");
    rebuild_title.set_halign(Align::Start);
    vbox.append(&rebuild_title);

    let rebuild_subtitle = Label::new(Some("Install pending kernel and driver updates"));
    rebuild_subtitle.add_css_class("dim-label");
    rebuild_subtitle.set_halign(Align::Start);
    vbox.append(&rebuild_subtitle);

    let rebuild_status = Label::new(Some(""));
    rebuild_status.set_halign(Align::Start);

    let rebuild_btn = Button::with_label("Apply Updates");
    rebuild_btn.set_halign(Align::Start);
    rebuild_btn.set_margin_top(4);

    {
        let rebuild_btn_ref = rebuild_btn.clone();
        let rebuild_status = rebuild_status.clone();
        let status_label = status_label.clone();

        rebuild_btn.connect_clicked(move |btn| {
            btn.set_sensitive(false);
            btn.set_label("Applying…");
            rebuild_status.set_text("Running system rebuild — this may take a few minutes…");
            status_label.set_text("System update in progress…");

            let (sender, receiver) = async_channel::bounded::<Result<(), String>>(1);
            std::thread::spawn(move || {
                let _ = sender.send_blocking(nix_ops::run_system_rebuild());
            });

            let btn2 = rebuild_btn_ref.clone();
            let rebuild_status2 = rebuild_status.clone();
            let status_label2 = status_label.clone();

            glib::MainContext::default().spawn_local(async move {
                if let Ok(result) = receiver.recv().await {
                    btn2.set_sensitive(true);
                    btn2.set_label("Apply Updates");
                    match result {
                        Ok(()) => {
                            rebuild_status2.set_text("Updates applied. Reboot to activate kernel/driver changes.");
                            status_label2.set_text("System updated.");
                        }
                        Err(ref e) => {
                            rebuild_status2.set_text(&format!("Update failed: {}", e));
                            status_label2.set_text("System update failed.");
                        }
                    }
                }
            });
        });
    }

    vbox.append(&rebuild_btn);
    vbox.append(&rebuild_status);

    // ── Generation List ───────────────────────────────────────────────────
    let sep2 = gtk4::Separator::new(Orientation::Horizontal);
    sep2.set_margin_top(8);
    sep2.set_margin_bottom(8);
    vbox.append(&sep2);

    let gen_title = Label::new(Some("System Generations"));
    gen_title.add_css_class("title-4");
    gen_title.set_halign(Align::Start);
    vbox.append(&gen_title);

    let gen_list_box = gtk4::ListBox::new();
    gen_list_box.set_selection_mode(gtk4::SelectionMode::None);
    gen_list_box.add_css_class("boxed-list");

    let gen_status = Label::new(Some(""));
    gen_status.set_halign(Align::Start);

    match nix_ops::list_generations() {
        Ok(gens) => {
            for gen in gens {
                let row = gtk4::Box::new(Orientation::Horizontal, 12);
                row.set_margin_top(6);
                row.set_margin_bottom(6);
                row.set_margin_start(8);
                row.set_margin_end(8);

                let info = Label::new(Some(&format!("#{} — {}", gen.id, gen.date)));
                info.set_halign(Align::Start);
                info.set_hexpand(true);
                row.append(&info);

                if gen.current {
                    let badge = Label::new(Some("current"));
                    badge.add_css_class("success");
                    badge.add_css_class("caption");
                    row.append(&badge);
                } else {
                    let rollback_btn = Button::with_label("Roll Back");
                    rollback_btn.add_css_class("destructive-action");
                    rollback_btn.set_halign(Align::End);

                    let gen_id = gen.id;
                    let gen_status_ref = gen_status.clone();
                    let status_label = status_label.clone();

                    rollback_btn.connect_clicked(move |btn| {
                        btn.set_sensitive(false);
                        btn.set_label("Rolling back…");
                        gen_status_ref.set_text(&format!("Rolling back to generation #{}…", gen_id));
                        status_label.set_text("Rollback in progress…");

                        let (sender, receiver) = async_channel::bounded::<Result<(), String>>(1);
                        std::thread::spawn(move || {
                            let _ = sender.send_blocking(nix_ops::run_system_rollback(gen_id));
                        });

                        let btn2 = btn.clone();
                        let gen_status2 = gen_status_ref.clone();
                        let status_label2 = status_label.clone();

                        glib::MainContext::default().spawn_local(async move {
                            if let Ok(result) = receiver.recv().await {
                                match result {
                                    Ok(()) => {
                                        gen_status2.set_text(&format!(
                                            "Rolled back to generation #{}. Reboot to activate.", gen_id
                                        ));
                                        status_label2.set_text("Rollback complete.");
                                    }
                                    Err(ref e) => {
                                        btn2.set_sensitive(true);
                                        btn2.set_label("Roll Back");
                                        gen_status2.set_text(&format!("Rollback failed: {}", e));
                                        status_label2.set_text("Rollback failed.");
                                    }
                                }
                            }
                        });
                    });

                    row.append(&rollback_btn);
                }

                let list_row = gtk4::ListBoxRow::new();
                list_row.set_child(Some(&row));
                gen_list_box.append(&list_row);
            }
        }
        Err(ref e) => {
            gen_status.set_text(&format!("Could not load generations: {}", e));
        }
    }

    vbox.append(&gen_list_box);
    vbox.append(&gen_status);

    vbox
}

fn update_apply_btn(btn: &Button, state: &Rc<RefCell<State>>) {
    let s = state.borrow();
    let has_adds = !s.staged_add.is_empty();
    let has_removes = !s.staged_remove.is_empty();
    btn.set_sensitive(has_adds || has_removes);
    btn.set_label(if has_adds && has_removes {
        "Install/Uninstall"
    } else if has_removes {
        "Uninstall"
    } else {
        "Install"
    });
}

fn rebuild_installed_list(
    installed_list: &ListBox,
    staged_list: &ListBox,
    state: &Rc<RefCell<State>>,
    apply_btn: &Button,
    update_all_btn: &Button,
    status_label: &Label,
) {
    while let Some(child) = installed_list.first_child() {
        installed_list.remove(&child);
    }
    let installed = state.borrow().installed.clone();
    for raw in &installed {
        if state.borrow().staged_remove.contains(raw) { continue; }
        let pname = raw.split('.').last().unwrap_or(raw.as_str()).to_string();
        let row = make_installed_row(
            pname, state.clone(), installed_list.clone(), staged_list.clone(),
            apply_btn.clone(), update_all_btn.clone(), status_label.clone(),
        );
        installed_list.append(&row);
    }
}

fn make_result_row(
    pkg: Package,
    state: Rc<RefCell<State>>,
    staged_list: ListBox,
    apply_btn: Button,
) -> ListBoxRow {
    let row = ListBoxRow::new();
    let hbox = GtkBox::new(Orientation::Horizontal, 8);
    hbox.set_margin_top(6);
    hbox.set_margin_bottom(6);
    hbox.set_margin_start(8);
    hbox.set_margin_end(8);

    let info = GtkBox::new(Orientation::Vertical, 2);
    info.set_hexpand(true);

    let version_str = if pkg.is_unstable {
        format!("{} · unstable", pkg.version)
    } else {
        pkg.version.clone()
    };
    let name_label = Label::new(Some(&format!("{} ({})", pkg.pname, version_str)));
    name_label.set_halign(Align::Start);
    info.append(&name_label);

    if !pkg.description.is_empty() {
        let desc = Label::new(Some(&pkg.description));
        desc.set_halign(Align::Start);
        desc.add_css_class("dim-label");
        desc.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        info.append(&desc);
    }

    hbox.append(&info);

    let is_installed = {
        let s = state.borrow();
        s.is_managed(&pkg.pname)
            && !s.staged_remove.contains(&pkg.pname)
            && !s.staged_remove.contains(&format!("unstable.{}", pkg.pname))
    };
    let is_staged = state.borrow().staged_add.contains_key(&pkg.pname);

    if is_installed {
        let check = Button::with_label("✓");
        check.set_valign(Align::Center);
        check.set_sensitive(false);
        check.add_css_class("suggested-action");
        hbox.append(&check);
    } else {
        let add_btn = Button::with_label(if is_staged { "–" } else { "+" });
        add_btn.set_valign(Align::Center);
        if is_staged { add_btn.add_css_class("suggested-action"); }

        {
            let pname = pkg.pname.clone();
            let pkg = pkg.clone();
            let state = state.clone();
            let staged_list = staged_list.clone();
            let apply_btn = apply_btn.clone();

            add_btn.connect_clicked(move |btn| {
                let currently_staged = state.borrow().staged_add.contains_key(&pname);
                if currently_staged {
                    state.borrow_mut().staged_add.remove(&pname);
                    remove_staged_row_by_name(&staged_list, &pname);
                    btn.set_label("+");
                    btn.remove_css_class("suggested-action");
                } else {
                    if state.borrow().staged_add.contains_key(&pname) { return; }
                    state.borrow_mut().staged_add.insert(pname.clone(), pkg.clone());
                    let staged_row = make_staged_install_row(
                        pname.clone(), state.clone(), staged_list.clone(),
                        apply_btn.clone(), btn.clone(),
                    );
                    staged_list.append(&staged_row);
                    btn.set_label("–");
                    btn.add_css_class("suggested-action");
                }
                update_apply_btn(&apply_btn, &state);
            });
        }

        hbox.append(&add_btn);
    }

    row.set_child(Some(&hbox));
    row
}

fn make_staged_install_row(
    pname: String,
    state: Rc<RefCell<State>>,
    staged_list: ListBox,
    apply_btn: Button,
    add_btn: Button,
) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.set_widget_name(&pname);

    let hbox = GtkBox::new(Orientation::Horizontal, 8);
    hbox.set_margin_top(4);
    hbox.set_margin_bottom(4);
    hbox.set_margin_start(8);
    hbox.set_margin_end(8);

    let badge = Label::new(Some("+"));
    badge.add_css_class("install-badge");
    badge.set_valign(Align::Center);
    hbox.append(&badge);

    let label = Label::new(Some(&pname));
    label.set_halign(Align::Start);
    label.set_hexpand(true);
    hbox.append(&label);

    let rm = Button::with_label("×");
    rm.add_css_class("destructive-action");
    rm.set_valign(Align::Center);

    {
        let pname = pname.clone();
        let state = state.clone();
        let staged_list = staged_list.clone();
        let row = row.clone();
        let apply_btn = apply_btn.clone();

        rm.connect_clicked(move |_| {
            state.borrow_mut().staged_add.remove(&pname);
            staged_list.remove(&row);
            add_btn.set_label("+");
            add_btn.remove_css_class("suggested-action");
            update_apply_btn(&apply_btn, &state);
        });
    }

    hbox.append(&rm);
    row.set_child(Some(&hbox));
    row
}

fn make_staged_remove_row(
    pname: String,
    raw: String,
    state: Rc<RefCell<State>>,
    staged_list: ListBox,
    installed_list: ListBox,
    apply_btn: Button,
    update_all_btn: Button,
    status_label: Label,
) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.set_widget_name(&pname);

    let hbox = GtkBox::new(Orientation::Horizontal, 8);
    hbox.set_margin_top(4);
    hbox.set_margin_bottom(4);
    hbox.set_margin_start(8);
    hbox.set_margin_end(8);

    let badge = Label::new(Some("−"));
    badge.add_css_class("remove-badge");
    badge.set_valign(Align::Center);
    hbox.append(&badge);

    let label = Label::new(Some(&pname));
    label.set_halign(Align::Start);
    label.set_hexpand(true);
    hbox.append(&label);

    let cancel = Button::with_label("×");
    cancel.set_valign(Align::Center);

    {
        let state = state.clone();
        let staged_list = staged_list.clone();
        let installed_list = installed_list.clone();
        let apply_btn = apply_btn.clone();
        let row = row.clone();

        cancel.connect_clicked(move |_| {
            state.borrow_mut().staged_remove.remove(&raw);
            staged_list.remove(&row);
            rebuild_installed_list(&installed_list, &staged_list, &state, &apply_btn, &update_all_btn, &status_label);
            update_apply_btn(&apply_btn, &state);
        });
    }

    hbox.append(&cancel);
    row.set_child(Some(&hbox));
    row
}

fn make_installed_row(
    pname: String,
    state: Rc<RefCell<State>>,
    installed_list: ListBox,
    staged_list: ListBox,
    apply_btn: Button,
    update_all_btn: Button,
    status_label: Label,
) -> ListBoxRow {
    let row = ListBoxRow::new();
    let hbox = GtkBox::new(Orientation::Horizontal, 8);
    hbox.set_margin_top(4);
    hbox.set_margin_bottom(4);
    hbox.set_margin_start(8);
    hbox.set_margin_end(8);

    let info = GtkBox::new(Orientation::Vertical, 2);
    info.set_hexpand(true);

    let has_update = state.borrow().has_update(&pname);

    let name_row = GtkBox::new(Orientation::Horizontal, 4);
    name_row.set_halign(Align::Start);
    let name_label = Label::new(Some(&pname));
    name_label.set_halign(Align::Start);
    name_row.append(&name_label);
    if has_update {
        let update_btn = Button::with_label("Update");
        update_btn.add_css_class("update-badge");
        update_btn.add_css_class("flat");
        update_btn.set_valign(Align::Center);

        {
            let pname = pname.clone();
            let state = state.clone();
            let installed_list = installed_list.clone();
            let staged_list = staged_list.clone();
            let apply_btn = apply_btn.clone();
            let update_all_btn = update_all_btn.clone();
            let status_label = status_label.clone();

            update_btn.connect_clicked(move |btn| {
                let available_ver = state.borrow().available_versions.get(&pname).cloned();
                let final_packages = state.borrow().installed.clone();
                let mut final_versions = state.borrow().installed_versions.clone();
                if let Some(ref ver) = available_ver {
                    final_versions.insert(pname.clone(), ver.clone());
                }

                btn.set_sensitive(false);
                update_all_btn.set_sensitive(false);
                status_label.set_text(&format!("Updating {}…", pname));

                let (sender, receiver) = async_channel::bounded::<Result<(), String>>(1);
                let pkgs = final_packages.clone();
                let vers = final_versions.clone();
                std::thread::spawn(move || {
                    let result = nix_ops::write_extra_packages(&pkgs, &vers)
                        .map_err(|e| e.to_string())
                        .and_then(|_| nix_ops::run_home_manager());
                    let _ = sender.send_blocking(result);
                });

                let btn2 = btn.clone();
                let update_all_btn2 = update_all_btn.clone();
                let status2 = status_label.clone();
                let state2 = state.clone();
                let installed2 = installed_list.clone();
                let staged2 = staged_list.clone();
                let apply_btn2 = apply_btn.clone();
                let pname2 = pname.clone();
                let available_ver2 = available_ver.clone();

                glib::MainContext::default().spawn_local(async move {
                    if let Ok(result) = receiver.recv().await {
                        match result {
                            Ok(()) => {
                                {
                                    let mut s = state2.borrow_mut();
                                    if let Some(ver) = available_ver2 {
                                        s.installed_versions.insert(pname2.clone(), ver);
                                    }
                                    s.available_versions.remove(&pname2);
                                }
                                status2.set_text(&format!("{} updated.", pname2));
                                rebuild_installed_list(&installed2, &staged2, &state2, &apply_btn2, &update_all_btn2, &status2);
                                update_all_btn2.set_sensitive(state2.borrow().any_updates());
                            }
                            Err(ref e) => {
                                status2.set_text(&format!("Update failed: {}", e));
                                btn2.set_sensitive(true);
                                update_all_btn2.set_sensitive(state2.borrow().any_updates());
                            }
                        }
                    }
                });
            });
        }

        name_row.append(&update_btn);
    }
    info.append(&name_row);

    let installed_v = state.borrow().installed_versions.get(&pname).cloned();
    let available_v = state.borrow().available_versions.get(&pname).cloned();

    if let Some(ref avail) = available_v {
        let version_text = match &installed_v {
            Some(inst) if inst != avail => format!("{} → {} available", inst, avail),
            Some(inst) => inst.clone(),
            None => format!("{} available", avail),
        };
        let version_label = Label::new(Some(&version_text));
        version_label.set_halign(Align::Start);
        version_label.add_css_class("dim-label");
        info.append(&version_label);
    } else if let Some(ref inst) = installed_v {
        let version_label = Label::new(Some(inst.as_str()));
        version_label.set_halign(Align::Start);
        version_label.add_css_class("dim-label");
        info.append(&version_label);
    }

    hbox.append(&info);

    let rm = Button::with_label("Uninstall");
    rm.add_css_class("destructive-action");
    rm.set_valign(Align::Center);

    {
        let pname = pname.clone();
        let state = state.clone();
        let installed_list = installed_list.clone();
        let staged_list = staged_list.clone();
        let row = row.clone();
        let apply_btn = apply_btn.clone();
        let update_all_btn = update_all_btn.clone();
        let status_label = status_label.clone();

        rm.connect_clicked(move |btn| {
            let window = btn.root().and_then(|r| r.downcast::<gtk4::Window>().ok());
            let dialog = gtk4::AlertDialog::builder()
                .message(&format!("Uninstall {}?", pname))
                .detail("This will remove it from your installed apps.")
                .buttons(["Cancel", "Uninstall"])
                .cancel_button(0)
                .default_button(0)
                .build();

            let pname = pname.clone();
            let state = state.clone();
            let installed_list = installed_list.clone();
            let staged_list = staged_list.clone();
            let apply_btn = apply_btn.clone();
            let update_all_btn = update_all_btn.clone();
            let status_label = status_label.clone();
            let row = row.clone();

            glib::MainContext::default().spawn_local(async move {
                if let Ok(1) = dialog.choose_future(window.as_ref()).await {
                    let raw = {
                        let s = state.borrow();
                        if s.installed.contains(&format!("unstable.{}", pname)) {
                            format!("unstable.{}", pname)
                        } else {
                            pname.clone()
                        }
                    };
                    state.borrow_mut().staged_remove.insert(raw.clone());
                    installed_list.remove(&row);

                    let staged_row = make_staged_remove_row(
                        pname.clone(), raw, state.clone(),
                        staged_list.clone(), installed_list.clone(), apply_btn.clone(),
                        update_all_btn.clone(), status_label.clone(),
                    );
                    staged_list.append(&staged_row);
                    update_apply_btn(&apply_btn, &state);
                }
            });
        });
    }

    hbox.append(&rm);
    row.set_child(Some(&hbox));
    row
}

fn remove_staged_row_by_name(staged_list: &ListBox, pname: &str) {
    let mut child = staged_list.first_child();
    while let Some(widget) = child {
        let next = widget.next_sibling();
        if widget.widget_name() == pname {
            staged_list.remove(&widget);
            break;
        }
        child = next;
    }
}
