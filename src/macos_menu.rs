//! Native macOS System Top Menu Bar
#[cfg(target_os = "macos")]
pub mod sys_menu {
    use objc2::sel;
    use objc2_app_kit::{NSApplication, NSMenu, NSMenuItem};
    use objc2_foundation::{MainThreadMarker, NSString};

    pub fn setup_main_menu(mtm: MainThreadMarker) {
        let app = NSApplication::sharedApplication(mtm);
        let menubar = NSMenu::new(mtm);

        // --- 1. Application Menu ("Well") ---
        let app_menu_item = NSMenuItem::new(mtm);
        let app_menu = NSMenu::new(mtm);

        let about_title = NSString::from_str("About Well");
        let about_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc(),
                &about_title,
                Some(sel!(orderFrontStandardAboutPanel:)),
                &NSString::from_str(""),
            )
        };
        app_menu.addItem(&about_item);

        app_menu.addItem(&NSMenuItem::separatorItem(mtm));

        let pref_title = NSString::from_str("Settings…");
        let pref_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc(),
                &pref_title,
                None,
                &NSString::from_str(","),
            )
        };
        app_menu.addItem(&pref_item);

        app_menu.addItem(&NSMenuItem::separatorItem(mtm));

        let hide_title = NSString::from_str("Hide Well");
        let hide_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc(),
                &hide_title,
                Some(sel!(hide:)),
                &NSString::from_str("h"),
            )
        };
        app_menu.addItem(&hide_item);

        let hide_others_title = NSString::from_str("Hide Others");
        let hide_others_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc(),
                &hide_others_title,
                Some(sel!(hideOtherApplications:)),
                &NSString::from_str("h"),
            )
        };
        app_menu.addItem(&hide_others_item);

        let show_all_title = NSString::from_str("Show All");
        let show_all_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc(),
                &show_all_title,
                Some(sel!(unhideAllApplications:)),
                &NSString::from_str(""),
            )
        };
        app_menu.addItem(&show_all_item);

        app_menu.addItem(&NSMenuItem::separatorItem(mtm));

        let quit_title = NSString::from_str("Quit Well");
        let quit_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc(),
                &quit_title,
                Some(sel!(terminate:)),
                &NSString::from_str("q"),
            )
        };
        app_menu.addItem(&quit_item);

        app_menu_item.setSubmenu(Some(&app_menu));
        menubar.addItem(&app_menu_item);

        // --- 2. Shell Menu ---
        let shell_menu_item = NSMenuItem::new(mtm);
        let shell_menu = NSMenu::new(mtm);
        shell_menu.setTitle(&NSString::from_str("Shell"));

        let new_tab_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc(),
                &NSString::from_str("New Tab"),
                None,
                &NSString::from_str("t"),
            )
        };
        shell_menu.addItem(&new_tab_item);

        let close_tab_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc(),
                &NSString::from_str("Close Tab"),
                None,
                &NSString::from_str("w"),
            )
        };
        shell_menu.addItem(&close_tab_item);

        shell_menu_item.setSubmenu(Some(&shell_menu));
        menubar.addItem(&shell_menu_item);

        // --- 3. Edit Menu ---
        let edit_menu_item = NSMenuItem::new(mtm);
        let edit_menu = NSMenu::new(mtm);
        edit_menu.setTitle(&NSString::from_str("Edit"));

        let copy_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc(),
                &NSString::from_str("Copy"),
                Some(sel!(copy:)),
                &NSString::from_str("c"),
            )
        };
        edit_menu.addItem(&copy_item);

        let paste_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc(),
                &NSString::from_str("Paste"),
                Some(sel!(paste:)),
                &NSString::from_str("v"),
            )
        };
        edit_menu.addItem(&paste_item);

        let select_all_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc(),
                &NSString::from_str("Select All"),
                Some(sel!(selectAll:)),
                &NSString::from_str("a"),
            )
        };
        edit_menu.addItem(&select_all_item);

        edit_menu_item.setSubmenu(Some(&edit_menu));
        menubar.addItem(&edit_menu_item);

        // --- 4. View Menu (Geometry Presets & Fullscreen) ---
        let view_menu_item = NSMenuItem::new(mtm);
        let view_menu = NSMenu::new(mtm);
        view_menu.setTitle(&NSString::from_str("View"));

        let r100_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc(),
                &NSString::from_str("100 × 30 (Default)"),
                None,
                &NSString::from_str(""),
            )
        };
        view_menu.addItem(&r100_item);

        let r80_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc(),
                &NSString::from_str("80 × 24 (VT100)"),
                None,
                &NSString::from_str(""),
            )
        };
        view_menu.addItem(&r80_item);

        let r120_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc(),
                &NSString::from_str("120 × 36 (Wide)"),
                None,
                &NSString::from_str(""),
            )
        };
        view_menu.addItem(&r120_item);

        view_menu.addItem(&NSMenuItem::separatorItem(mtm));

        let fullscreen_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc(),
                &NSString::from_str("Toggle Full Screen"),
                Some(sel!(toggleFullScreen:)),
                &NSString::from_str("f"),
            )
        };
        view_menu.addItem(&fullscreen_item);

        view_menu_item.setSubmenu(Some(&view_menu));
        menubar.addItem(&view_menu_item);

        // --- 5. Window Menu ---
        let window_menu_item = NSMenuItem::new(mtm);
        let window_menu = NSMenu::new(mtm);
        window_menu.setTitle(&NSString::from_str("Window"));

        let min_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc(),
                &NSString::from_str("Minimize"),
                Some(sel!(performMiniaturize:)),
                &NSString::from_str("m"),
            )
        };
        window_menu.addItem(&min_item);

        let zoom_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc(),
                &NSString::from_str("Zoom"),
                Some(sel!(performZoom:)),
                &NSString::from_str(""),
            )
        };
        window_menu.addItem(&zoom_item);

        window_menu_item.setSubmenu(Some(&window_menu));
        menubar.addItem(&window_menu_item);

        app.setMainMenu(Some(&menubar));
    }
}
