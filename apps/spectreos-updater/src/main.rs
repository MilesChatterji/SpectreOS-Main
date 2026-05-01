use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, Label, ListBox,
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
    installed: Vec<String>,      // updater-managed block in home.nix (shown in managed list)
    all_installed: Vec<String>,  // all home.packages in home.nix (for ✓ in search results)
    staged_add: HashMap<String, Package>,
    staged_remove: HashSet<String>,
}

impl State {
    fn needs_apply(&self) -> bool {
        !self.staged_add.is_empty() || !self.staged_remove.is_empty()
    }

    fn final_packages(&self) -> Vec<String> {
        let mut pkgs: Vec<String> = self.installed
            .iter()
            .filter(|p| !self.staged_remove.contains(*p))
            .cloned()
            .collect();
        for pname in self.staged_add.keys() {
            if !pkgs.contains(pname) {
                pkgs.push(pname.clone());
            }
        }
        pkgs.sort();
        pkgs
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
        all_installed: nix_ops::read_all_home_packages(),
        staged_add: HashMap::new(),
        staged_remove: HashSet::new(),
    }));

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

    let search = SearchEntry::new();
    search.set_placeholder_text(Some("Search nixpkgs — e.g. helix, micro, obsidian"));
    root.append(&search);

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

    let staged_label = Label::new(Some("Staged to Add"));
    staged_label.set_halign(Align::Start);
    root.append(&staged_label);

    let staged_list = ListBox::new();
    staged_list.set_selection_mode(SelectionMode::None);
    let sp = Label::new(Some("No packages staged"));
    staged_list.set_placeholder(Some(&sp));
    let staged_scroll = ScrolledWindow::builder()
        .min_content_height(80)
        .vexpand(false)
        .hscrollbar_policy(PolicyType::Never)
        .child(&staged_list)
        .build();
    root.append(&staged_scroll);

    root.append(&Separator::new(Orientation::Horizontal));

    let installed_label = Label::new(Some("Managed by SpectreOS"));
    installed_label.set_halign(Align::Start);
    root.append(&installed_label);

    let installed_list = ListBox::new();
    installed_list.set_selection_mode(SelectionMode::None);
    let ip = Label::new(Some("No packages managed yet"));
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
    let apply_btn = Button::with_label("Apply Changes");
    apply_btn.add_css_class("suggested-action");
    apply_btn.set_sensitive(false);
    bottom_bar.append(&apply_btn);
    root.append(&bottom_bar);

    window.set_child(Some(&root));

    // Populate installed list from persisted state
    {
        let installed = state.borrow().installed.clone();
        for pname in &installed {
            let row = make_installed_row(
                pname.clone(), state.clone(), installed_list.clone(), apply_btn.clone(),
            );
            installed_list.append(&row);
        }
    }

    window.present();

    // Quit
    {
        let window = window.clone();
        quit_btn.connect_clicked(move |_| window.close());
    }

    // Search
    {
        let results_list = results_list.clone();
        let staged_list = staged_list.clone();
        let state = state.clone();
        let apply_btn = apply_btn.clone();

        search.connect_search_changed(move |entry| {
            let query = entry.text().to_string();
            while let Some(child) = results_list.first_child() {
                results_list.remove(&child);
            }
            if query.trim().is_empty() { return; }

            let loading = ListBoxRow::new();
            loading.set_child(Some(&Label::new(Some("Searching…"))));
            results_list.append(&loading);

            let (sender, receiver) = async_channel::bounded::<Vec<Package>>(1);
            std::thread::spawn(move || {
                let _ = sender.send_blocking(nix_ops::search(&query));
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
        });
    }

    // Apply
    {
        let state = state.clone();
        let status_label = status_label.clone();
        let apply_btn_ref = apply_btn.clone();
        let staged_list = staged_list.clone();
        let installed_list = installed_list.clone();
        let search = search.clone();
        let results_list = results_list.clone();

        apply_btn.connect_clicked(move |btn| {
            if !state.borrow().needs_apply() { return; }
            let final_packages = state.borrow().final_packages();

            btn.set_sensitive(false);
            btn.set_label("Applying…");
            status_label.set_text("Running home-manager switch…");

            let (sender, receiver) = async_channel::bounded::<Result<(), String>>(1);
            let pkgs = final_packages.clone();
            std::thread::spawn(move || {
                let result = nix_ops::write_extra_packages(&pkgs)
                    .map_err(|e| e.to_string())
                    .and_then(|_| nix_ops::run_home_manager());
                let _ = sender.send_blocking(result);
            });

            let btn2 = apply_btn_ref.clone();
            let status2 = status_label.clone();
            let state2 = state.clone();
            let staged2 = staged_list.clone();
            let installed2 = installed_list.clone();
            let search2 = search.clone();
            let results2 = results_list.clone();

            glib::MainContext::default().spawn_local(async move {
                if let Ok(result) = receiver.recv().await {
                    btn2.set_label("Apply Changes");
                    match result {
                        Ok(()) => {
                            status2.set_text("Done! Changes applied.");

                            {
                                let mut s = state2.borrow_mut();
                                s.installed = final_packages.clone();
                                s.all_installed = nix_ops::read_all_home_packages();
                                s.staged_add.clear();
                                s.staged_remove.clear();
                            }

                            // Clear staged list
                            while let Some(child) = staged2.first_child() {
                                staged2.remove(&child);
                            }

                            // Rebuild managed list
                            while let Some(child) = installed2.first_child() {
                                installed2.remove(&child);
                            }
                            for pname in &final_packages {
                                let row = make_installed_row(
                                    pname.clone(), state2.clone(), installed2.clone(), btn2.clone(),
                                );
                                installed2.append(&row);
                            }

                            // Clear search so re-search shows fresh ✓ marks
                            search2.set_text("");
                            while let Some(child) = results2.first_child() {
                                results2.remove(&child);
                            }

                            btn2.set_sensitive(false);
                        }
                        Err(ref e) => {
                            status2.set_text(&format!("Error: {}", e));
                            btn2.set_sensitive(true);
                        }
                    }
                }
            });
        });
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

    let name_label = Label::new(Some(&format!("{} ({})", pkg.pname, pkg.version)));
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
        s.all_installed.contains(&pkg.pname) && !s.staged_remove.contains(&pkg.pname)
    };
    let is_staged = state.borrow().staged_add.contains_key(&pkg.pname);

    if is_installed {
        let check = Button::with_label("✓");
        check.set_valign(Align::Center);
        check.set_sensitive(false);
        check.add_css_class("suggested-action");
        hbox.append(&check);
    } else {
        // Start as "+" or "–" depending on whether it's already staged
        let add_btn = Button::with_label(if is_staged { "–" } else { "+" });
        add_btn.set_valign(Align::Center);
        if is_staged {
            add_btn.add_css_class("suggested-action");
        }

        {
            let pname = pkg.pname.clone();
            let pkg = pkg.clone();
            let state = state.clone();
            let staged_list = staged_list.clone();
            let apply_btn = apply_btn.clone();

            add_btn.connect_clicked(move |btn| {
                let currently_staged = state.borrow().staged_add.contains_key(&pname);
                if currently_staged {
                    // Unstage: remove from state and remove the staged row
                    state.borrow_mut().staged_add.remove(&pname);
                    remove_staged_row_by_name(&staged_list, &pname);
                    btn.set_label("+");
                    btn.remove_css_class("suggested-action");
                    if !state.borrow().needs_apply() {
                        apply_btn.set_sensitive(false);
                    }
                } else {
                    // Stage it
                    if state.borrow().staged_add.contains_key(&pname) { return; }
                    state.borrow_mut().staged_add.insert(pname.clone(), pkg.clone());
                    let staged_row = make_staged_row(
                        pname.clone(), state.clone(), staged_list.clone(),
                        apply_btn.clone(), btn.clone(),
                    );
                    staged_list.append(&staged_row);
                    btn.set_label("–");
                    btn.add_css_class("suggested-action");
                    apply_btn.set_sensitive(true);
                }
            });
        }

        hbox.append(&add_btn);
    }

    row.set_child(Some(&hbox));
    row
}

fn make_staged_row(
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
            // Reset the corresponding search result button if it's still visible
            add_btn.set_label("+");
            add_btn.remove_css_class("suggested-action");
            if !state.borrow().needs_apply() {
                apply_btn.set_sensitive(false);
            }
        });
    }

    hbox.append(&rm);
    row.set_child(Some(&hbox));
    row
}

fn make_installed_row(
    pname: String,
    state: Rc<RefCell<State>>,
    installed_list: ListBox,
    apply_btn: Button,
) -> ListBoxRow {
    let row = ListBoxRow::new();
    let hbox = GtkBox::new(Orientation::Horizontal, 8);
    hbox.set_margin_top(4);
    hbox.set_margin_bottom(4);
    hbox.set_margin_start(8);
    hbox.set_margin_end(8);

    let label = Label::new(Some(&pname));
    label.set_halign(Align::Start);
    label.set_hexpand(true);
    hbox.append(&label);

    let rm = Button::with_label("Remove");
    rm.add_css_class("destructive-action");
    rm.set_valign(Align::Center);

    {
        let pname = pname.clone();
        let state = state.clone();
        let installed_list = installed_list.clone();
        let row = row.clone();
        let apply_btn = apply_btn.clone();

        rm.connect_clicked(move |_| {
            state.borrow_mut().staged_remove.insert(pname.clone());
            installed_list.remove(&row);
            apply_btn.set_sensitive(true);
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
