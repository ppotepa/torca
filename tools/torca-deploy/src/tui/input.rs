use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};

#[derive(Default)]
pub struct InputGuard {
    last_press: Option<(KeyCode, Instant)>,
}

impl InputGuard {
    pub fn read(&mut self) -> io::Result<Option<KeyCode>> {
        loop {
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if let Some(key) = self.accept(key) {
                return Ok(Some(key));
            }
        }
    }

    pub(crate) fn accept(&mut self, key: KeyEvent) -> Option<KeyCode> {
        match key.kind {
            KeyEventKind::Release => {
                if self.last_press.map(|(code, _)| code) == Some(key.code) {
                    self.last_press = None;
                }
                None
            }
            KeyEventKind::Repeat => None,
            KeyEventKind::Press => {
                let now = Instant::now();
                let navigation = matches!(
                    key.code,
                    KeyCode::Up
                        | KeyCode::Down
                        | KeyCode::Left
                        | KeyCode::Right
                        | KeyCode::Tab
                        | KeyCode::BackTab
                );
                if navigation
                    && self.last_press.is_some_and(|(code, at)| {
                        code == key.code && now.duration_since(at) < Duration::from_millis(220)
                    })
                {
                    return None;
                }
                self.last_press = Some((key.code, now));
                Some(key.code)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_navigation_key_is_ignored() {
        let mut guard = InputGuard::default();
        let key = KeyEvent::new(KeyCode::Down, crossterm::event::KeyModifiers::NONE);
        assert_eq!(guard.accept(key), Some(KeyCode::Down));
        assert_eq!(
            guard.accept(KeyEvent::new_with_kind(
                KeyCode::Down,
                crossterm::event::KeyModifiers::NONE,
                KeyEventKind::Repeat,
            )),
            None
        );
    }

    #[test]
    fn shift_tab_backtab_is_treated_as_navigation_and_repeat_safe() {
        let mut guard = InputGuard::default();
        let key = KeyEvent::new(KeyCode::BackTab, crossterm::event::KeyModifiers::SHIFT);
        assert_eq!(guard.accept(key), Some(KeyCode::BackTab));
        assert_eq!(
            guard.accept(KeyEvent::new_with_kind(
                KeyCode::BackTab,
                crossterm::event::KeyModifiers::SHIFT,
                KeyEventKind::Repeat,
            )),
            None
        );
    }
}
