use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, CheckButton, Label, ListBox,
    ListBoxRow, Orientation, PolicyType, ScrolledWindow, SearchEntry, SelectionMode,
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
    let app = Application::builder()
        .application_id(APP_ID)
        .build();
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

    // CSS for update badge and staging indicators
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

    let search_row = GtkBox::new(Orientation::Horizontal, 8);
    let search = SearchEntry::new();
    search.set_hexpand(true);
    search.set_placeholder_text(Some("Search nixpkgs — e.g. helix, micro, obsidian"));
    search_row.append(&search);
    let unstable_check = CheckButton::with_label("Include unstable");
    unstable_check.set_valign(Align::Center);
    search_row.append(&unstable_check);
    root.append(&search_row);

    let results_label = Label::new(Some("Results"));
    results_label.set_halign(Align::Start);
    root.append(&results_label);

    let results_list = ListBox::new();
    results_list.set_selection_mode(SelectionMode::None);
    let rp = Label::new(Some("Search for a package above"));
    results_list.set_placeholder(Some(&rp));
    let results_scroll = ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(PolicyType::Never)
        .child(&results_list)
        .build();
    root.append(&results_scroll);

    root.append(&Separator::new(Orientation::Horizontal));

    let staged_label = Label::new(Some("Install/Uninstall Staging"));
    staged_label.set_halign(Align::Start);
    root.append(&staged_label);

    let staged_list = ListBox::new();
    staged_list.set_selection_mode(SelectionMode::None);
    let sp = Label::new(Some("No changes staged"));
    staged_list.set_placeholder(Some(&sp));
    let staged_scroll = ScrolledWindow::builder()
        .min_content_height(80)
        .vexpand(false)
        .hscrollbar_policy(PolicyType::Never)
        .child(&staged_list)
        .build();
    root.append(&staged_scroll);

    root.append(&Separator::new(Orientation::Horizontal));

    let installed_label = Label::new(Some("Installed Apps"));
    installed_label.set_halign(Align::Start);
    root.append(&installed_label);

    let installed_list = ListBox::new();
    installed_list.set_selection_mode(SelectionMode::None);
    let ip = Label::new(Some("No packages installed yet"));
    installed_list.set_placeholder(Some(&ip));
    let installed_scroll = ScrolledWindow::builder()
        .min_content_height(100)
        .vexpand(false)
        .hscrollbar_policy(PolicyType::Never)
        .child(&installed_list)
        .build();
    root.append(&installed_scroll);

    let status_label = Label::new(Some(""));
    status_label.set_halign(Align::Start);
    root.append(&status_label);

    let bottom_bar = GtkBox::new(Orientation::Horizontal, 8);
    let quit_btn = Button::with_label("Quit");
    bottom_bar.append(&quit_btn);
    let spacer = Label::new(None);
    spacer.set_hexpand(true);
    bottom_bar.append(&spacer);
    let update_all_btn = Button::with_label("Update All");
    update_all_btn.add_css_class("suggested-action");
    update_all_btn.set_sensitive(false);
    bottom_bar.append(&update_all_btn);
    let apply_btn = Button::with_label("Install");
    apply_btn.add_css_class("suggested-action");
    apply_btn.set_sensitive(false);
    bottom_bar.append(&apply_btn);
    root.append(&bottom_bar);

    window.set_child(Some(&root));

    rebuild_installed_list(&installed_list, &staged_list, &state, &apply_btn);

    window.present();

    // Quit
    {
        let window = window.clone();
        quit_btn.connect_clicked(move |_| window.close());
    }

    // Search
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

    // Channel update on launch
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

            let (sender, receiver) =
                async_channel::bounded::<(Result<(), String>, HashMap<String, String>)>(1);

            std::thread::spawn(move || {
                let channel_result = nix_ops::run_channel_update();
                let available = nix_ops::fetch_available_versions(&managed);
                let _ = sender.send_blocking((channel_result, available));
            });

            glib::MainContext::default().spawn_local(async move {
                if let Ok((_channel_result, available)) = receiver.recv().await {
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

                    rebuild_installed_list(&installed_list, &staged_list, &state, &apply_btn);
                }
            });
        }
    }

    // Apply staged changes
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

                            rebuild_installed_list(&installed2, &staged2, &state2, &btn2);
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

    // Update All
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
                            rebuild_installed_list(&installed2, &staged2, &state2, &apply_btn2);
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

/// Update the Install/Uninstall button label and sensitivity based on staged state.
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
) {
    while let Some(child) = installed_list.first_child() {
        installed_list.remove(&child);
    }
    let installed = state.borrow().installed.clone();
    for raw in &installed {
        // Items staged for removal are shown in the staging area, not here.
        if state.borrow().staged_remove.contains(raw) { continue; }
        let pname = raw.split('.').last().unwrap_or(raw.as_str()).to_string();
        let row = make_installed_row(
            pname, state.clone(), installed_list.clone(), staged_list.clone(), apply_btn.clone(),
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

/// Staged row for a package being INSTALLED (shows green + badge).
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

/// Staged row for a package being REMOVED (shows red − badge).
/// The × button restores the package to the installed list.
fn make_staged_remove_row(
    pname: String,
    raw: String,
    state: Rc<RefCell<State>>,
    staged_list: ListBox,
    installed_list: ListBox,
    apply_btn: Button,
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
            rebuild_installed_list(&installed_list, &staged_list, &state, &apply_btn);
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

    // Name row — includes blue "Update" pill when an update is available.
    let name_row = GtkBox::new(Orientation::Horizontal, 4);
    name_row.set_halign(Align::Start);
    let name_label = Label::new(Some(&pname));
    name_label.set_halign(Align::Start);
    name_row.append(&name_label);
    if has_update {
        let badge = Label::new(Some("Update"));
        badge.add_css_class("update-badge");
        badge.set_valign(Align::Center);
        name_row.append(&badge);
    }
    info.append(&name_row);

    // Version info row (dim).
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

    let rm = Button::with_label("Remove");
    rm.add_css_class("destructive-action");
    rm.set_valign(Align::Center);

    {
        let pname = pname.clone();
        let state = state.clone();
        let installed_list = installed_list.clone();
        let staged_list = staged_list.clone();
        let row = row.clone();
        let apply_btn = apply_btn.clone();

        rm.connect_clicked(move |_| {
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
            );
            staged_list.append(&staged_row);
            update_apply_btn(&apply_btn, &state);
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
