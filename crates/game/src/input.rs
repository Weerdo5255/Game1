use macroquad::prelude::*;

/// One frame of input in logical 1280x720 coordinates. Everything reads this, never macroquad directly,
/// so a script can drive the UI.
#[derive(Clone, Default)]
#[allow(dead_code)] // `wheel` is read by the log panel
pub struct Input {
    pub mouse: Vec2,
    pub left_pressed: bool,
    pub left_released: bool,
    pub left_down: bool,
    pub right_pressed: bool,
    pub right_released: bool,
    /// -1, 0 or +1.
    pub wheel: i32,
    pub keys: Vec<KeyCode>,
}

impl Input {
    pub fn poll(scale: f32, ox: f32, oy: f32) -> Input {
        let (mx, my) = mouse_position();
        let wheel = mouse_wheel().1;
        Input {
            mouse: vec2((mx - ox) / scale, (my - oy) / scale),
            left_pressed: is_mouse_button_pressed(MouseButton::Left),
            left_released: is_mouse_button_released(MouseButton::Left),
            left_down: is_mouse_button_down(MouseButton::Left),
            right_pressed: is_mouse_button_pressed(MouseButton::Right),
            right_released: is_mouse_button_released(MouseButton::Right),
            wheel: if wheel > 0.0 { 1 } else if wheel < 0.0 { -1 } else { 0 },
            keys: get_keys_pressed().into_iter().collect(),
        }
    }
    pub fn key(&self, k: KeyCode) -> bool {
        self.keys.contains(&k)
    }
    pub fn over(&self, r: Rect) -> bool {
        r.contains(self.mouse)
    }
}

pub fn key_from_name(name: &str) -> Option<KeyCode> {
    use KeyCode::*;
    const TABLE: &[(&str, KeyCode)] = &[
        ("A", A), ("B", B), ("C", C), ("D", D), ("E", E), ("F", F), ("G", G), ("H", H), ("I", I), ("J", J),
        ("K", K), ("L", L), ("M", M), ("N", N), ("O", O), ("P", P), ("Q", Q), ("R", R), ("S", S), ("T", T),
        ("U", U), ("V", V), ("W", W), ("X", X), ("Y", Y), ("Z", Z), ("Key0", Key0), ("Key1", Key1),
        ("Key2", Key2), ("Key3", Key3), ("Key4", Key4), ("Key5", Key5), ("Key6", Key6), ("Key7", Key7),
        ("Key8", Key8), ("Key9", Key9), ("Space", Space), ("Escape", Escape), ("Tab", Tab), ("Enter", Enter),
        ("F1", F1), ("F2", F2), ("F3", F3), ("F4", F4), ("F5", F5), ("F6", F6), ("F7", F7), ("F8", F8),
        ("F9", F9), ("F10", F10), ("F11", F11), ("F12", F12), ("Left", Left), ("Right", Right), ("Up", Up),
        ("Down", Down),
    ];
    TABLE.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|&(_, k)| k)
}

enum Cmd {
    Move(f32, f32),
    Press(f32, f32, bool),
    Release(bool),
    Key(KeyCode),
    Wait(f64),
    Shot(String),
    Quit,
}

/// `--script "move X Y; click X Y; rclick X Y; key NAME; wait SECS; shot PATH; quit"`.
/// One command per frame; a click is a press frame followed by a release frame.
pub struct Script {
    cmds: Vec<Cmd>,
    next: usize,
    wait_until: Option<f64>,
    mouse: Vec2,
    down: bool,
}

pub struct ScriptFrame {
    pub input: Input,
    pub shot: Option<String>,
    pub quit: bool,
}

impl Script {
    pub fn parse(src: &str) -> Script {
        let mut cmds = Vec::new();
        for part in src.split(';') {
            let words: Vec<&str> = part.split_whitespace().collect();
            let num = |i: usize| words.get(i).and_then(|w| w.parse::<f32>().ok()).unwrap_or(0.0);
            match words.first().copied() {
                Some("move") => cmds.push(Cmd::Move(num(1), num(2))),
                Some("click") => {
                    cmds.push(Cmd::Press(num(1), num(2), false));
                    cmds.push(Cmd::Release(false));
                }
                Some("rclick") => {
                    cmds.push(Cmd::Press(num(1), num(2), true));
                    cmds.push(Cmd::Release(true));
                }
                Some("key") => {
                    if let Some(k) = words.get(1).and_then(|w| key_from_name(w)) {
                        cmds.push(Cmd::Key(k));
                    }
                }
                Some("wait") => cmds.push(Cmd::Wait(num(1) as f64)),
                Some("shot") => cmds.push(Cmd::Shot(words.get(1).copied().unwrap_or("shot.png").to_owned())),
                Some("quit") => cmds.push(Cmd::Quit),
                _ => {}
            }
        }
        Script { cmds, next: 0, wait_until: None, mouse: vec2(-10.0, -10.0), down: false }
    }

    pub fn frame(&mut self, now: f64) -> ScriptFrame {
        let mut out = ScriptFrame { input: Input { mouse: self.mouse, left_down: self.down, ..Default::default() }, shot: None, quit: false };
        if let Some(t) = self.wait_until {
            if now < t {
                return out;
            }
            self.wait_until = None;
        }
        let Some(cmd) = self.cmds.get(self.next) else { return out };
        self.next += 1;
        match cmd {
            Cmd::Move(x, y) => self.mouse = vec2(*x, *y),
            Cmd::Press(x, y, right) => {
                self.mouse = vec2(*x, *y);
                if *right {
                    out.input.right_pressed = true;
                } else {
                    out.input.left_pressed = true;
                    self.down = true;
                }
            }
            Cmd::Release(right) => {
                if *right {
                    out.input.right_released = true;
                } else {
                    out.input.left_released = true;
                    self.down = false;
                }
            }
            Cmd::Key(k) => out.input.keys.push(*k),
            Cmd::Wait(secs) => self.wait_until = Some(now + secs),
            Cmd::Shot(path) => out.shot = Some(path.clone()),
            Cmd::Quit => out.quit = true,
        }
        out.input.mouse = self.mouse;
        out.input.left_down = self.down;
        out
    }
}
