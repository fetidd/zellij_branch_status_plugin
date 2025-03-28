use zellij_tile::prelude::*;

use std::collections::BTreeMap;
use std::ops::Deref;
use std::sync::LazyLock;

use regex;

static GIT_BRANCH_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"\* (?<branch>[a-zA-Z0-9_-]+)").expect("bad regex"));
static SVN_BRANCH_REGEX: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"URL:.+branches/(?<branch>[a-zA-Z0-9_-]+)").expect("bad regex")
});
static _GIT_DIRTY_REGEX: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"nothing to commit, working tree clean").expect("bad regex")
});

const GIT_QUERY_CMD: [&'static str; 2] = ["git", "branch"];
const SVN_QUERY_CMD: [&'static str; 2] = ["svn", "info"];

#[derive(Default)]
struct State {
    // the state of the plugin
    current_branch: Option<String>,
    is_dirty: bool,
    focused_tab: Option<TabInfo>,
    focused_pane: Option<PaneInfo>,
    query_cmd: &'static [&'static str],
    branch_regex: Option<&'static LazyLock<regex::Regex>>,
    dirty_regex: Option<&'static LazyLock<regex::Regex>>,
}

register_plugin!(State);

impl State {
    fn set_git(&mut self) {
        self.branch_regex = Some(&GIT_BRANCH_REGEX);
        self.dirty_regex = None;
        self.query_cmd = &GIT_QUERY_CMD;
    }
    fn set_svn(&mut self) {
        self.branch_regex = Some(&SVN_BRANCH_REGEX);
        self.dirty_regex = None;
        self.query_cmd = &SVN_QUERY_CMD;
    }
}

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        set_selectable(false);
        match configuration.get("vcs_type") {
            Some(vcs) => match vcs.as_str() {
                "svn" => self.set_svn(),
                "git" => self.set_git(),
                _ => panic!("Invalid vcs_type"),
            },
            None => self.set_git(),
        }
        request_permission(&[
            PermissionType::RunCommands,
            PermissionType::ReadApplicationState,
        ]);
        subscribe(&[
            EventType::RunCommandResult,
            EventType::Timer,
            EventType::PaneUpdate,
            EventType::TabUpdate,
        ]);
        set_timeout(1.0);
    }

    fn update(&mut self, event: Event) -> bool {
        let mut should_render = false;
        let query_branch = || run_command(&self.query_cmd, BTreeMap::new());
        match event {
            Event::Timer(_) => {
                // dbg!("timer elapsed, refreshing...");
                set_timeout(3.0);
                query_branch();
            }
            Event::PaneUpdate(pm) => {
                let tab = match &self.focused_tab {
                    Some(focused) => focused.position,
                    None => 1,
                };
                if let Some(pane) = get_focused_pane(tab, &pm) {
                    self.focused_pane = Some(pane);
                    dbg!(&self.focused_pane);
                }
            }
            Event::TabUpdate(tabs) => {
                self.focused_tab = get_focused_tab(&tabs);
            }
            Event::RunCommandResult(_code, stdout, _stderr, _ctx) => {
                let output = String::from_utf8(stdout).expect("bad command output");
                let r: &regex::Regex = self.branch_regex.expect("no pattern!").deref();
                if let Some(caps) = &r.captures(&output) {
                    let branch = &caps["branch"];
                    self.current_branch = Some(branch.into());
                    should_render = true;
                }
            }
            _ => {}
        };
        should_render
    }

    fn pipe(&mut self, _pipe_message: PipeMessage) -> bool {
        #[allow(unused_mut)]
        let mut should_render = false;
        // react to data piped to this plugin from the CLI, a keybinding or another plugin
        // read more about pipes: https://zellij.dev/documentation/plugin-pipes
        // return true if this plugin's `render` function should be called for the plugin to render
        // itself
        // keybind to refresh the branch?
        should_render
    }

    fn render(&mut self, _rows: usize, _cols: usize) {
        match &self.current_branch {
            Some(branch) => {
                let mut branch_text = String::from('\u{e0a0}');
                branch_text.push_str(" ");
                branch_text.push_str(branch);
                if self.is_dirty {
                    branch_text.push_str("*");
                }
                let output = Text::new(branch_text);
                print_text(output);
            }
            None => print_text(Text::new("Failed to get branch, check logs!")), // TODO better handling here, dont want to show this when it first loads up!
        };
    }
}
