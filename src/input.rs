use std::num::IntErrorKind;

use ratatui::{
    crossterm::event::KeyCode,
    style::{Color, Stylize as _},
    text::{Line, Span},
};

use crate::{Error, cursor::Cursor, model::Model};

#[derive(Default)]
pub struct InputHandler {
    key_handler: Option<Box<dyn KeyPressHandler>>,
}
impl InputHandler {
    pub fn status<'a>(&'a self) -> Option<Line<'a>> {
        self.key_handler
            .as_deref()
            .and_then(KeyPressHandler::status)
    }

    pub fn on_key_press(&mut self, model: &mut Model, key: KeyCode) -> Result<bool, Error> {
        match self
            .key_handler
            .as_deref_mut()
            .unwrap_or(&mut NormalHandler)
            .handle(model, key)?
        {
            Response::Ignored => return Ok(false),
            Response::Ok => {}
            Response::Pop => {
                assert!(self.key_handler.is_some(), "default handler shouldn't pop");
                self.key_handler = None;
            }
            Response::PopWith(key) => {
                assert!(self.key_handler.is_some(), "default handler shouldn't pop");
                self.key_handler = None;
                return self.on_key_press(model, key);
            }
            Response::Set(mut handler) => {
                handler.enter(model)?;
                self.key_handler = Some(handler);
            }
        }
        Ok(true)
    }
}

#[derive(Default)]
enum Response {
    #[default]
    Ok,
    Ignored,
    Pop,
    PopWith(KeyCode),
    Set(Box<dyn KeyPressHandler>),
}
trait KeyPressHandler {
    fn status<'a>(&'a self) -> Option<Line<'a>>;
    fn enter(&mut self, model: &mut Model) -> Result<(), Error>;
    fn handle(&mut self, model: &mut Model, key: KeyCode) -> Result<Response, Error>;
}

struct NormalHandler;
impl KeyPressHandler for NormalHandler {
    fn status<'a>(&'a self) -> Option<Line<'a>> {
        None
    }
    fn enter(&mut self, _model: &mut Model) -> Result<(), Error> {
        Ok(())
    }

    fn handle(&mut self, model: &mut Model, key: KeyCode) -> Result<Response, Error> {
        let page_height = 10; // TODO: model.inner_height(todo!()) as i16 - 2;

        match key {
            KeyCode::Char(x) if x.is_ascii_digit() && x != '0' => {
                return Ok(Response::Set(Box::new(CountMovesHandler {
                    state: x.to_string(),
                })));
            }
            KeyCode::Char('r') => {
                todo!("reload model");
            }
            KeyCode::Char('j') | KeyCode::Down => {
                model.scroll_by(1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                model.scroll_by(-1);
            }
            KeyCode::Char('d') => {
                model.scroll_by((page_height + 1) / 2);
            }
            KeyCode::Char('u') => {
                model.scroll_by(-(page_height + 1) / 2);
            }
            KeyCode::Char('f' | ' ') | KeyCode::PageDown => {
                model.scroll_by(page_height);
            }
            KeyCode::Char('b') | KeyCode::PageUp => {
                model.scroll_by(-page_height);
            }
            KeyCode::Char('g') => {
                model.scroll = 0;
            }
            KeyCode::Char('G') => {
                model.scroll_by(i16::MAX);
            }
            KeyCode::Char('/') => {
                return Ok(Response::Set(Box::<SearchHandler>::default()));
            }
            KeyCode::Char('n') => {
                model.cursor_next();
            }
            KeyCode::Char('N') => {
                model.cursor_prev();
            }
            KeyCode::F(11) => {
                model.log_snapshot = match model.log_snapshot {
                    None => Some(flexi_logger::Snapshot::new()),
                    _ => None,
                };
            }
            KeyCode::Enter => {
                todo!("try open link");
            }
            KeyCode::Esc => {
                model.cursor = Cursor::None;
                model.add_searches(None);
            }
            _ if model.movement_count == 0 => return Ok(Response::Ignored),
            _ => {}
        }
        model.movement_count = 0;
        Ok(Response::Ok)
    }
}

struct CountMovesHandler {
    state: String,
}
impl CountMovesHandler {
    fn count(&self) -> usize {
        match self.state.parse::<usize>() {
            Ok(x) => {
                assert!(x > 0, "self.state should always start with a non-zero");
                x
            }
            Err(e) => {
                assert!(
                    matches!(e.kind(), IntErrorKind::PosOverflow),
                    "self.state should contain only ascii digits"
                );
                usize::MAX
            }
        }
    }
}
impl KeyPressHandler for CountMovesHandler {
    fn status<'a>(&'a self) -> Option<Line<'a>> {
        let mut line = Line::default();
        line.spans.push(Span::from(&self.state));
        Some(line)
    }

    fn enter(&mut self, model: &mut Model) -> Result<(), Error> {
        model.movement_count = 0;
        Ok(())
    }

    fn handle(&mut self, model: &mut Model, key: KeyCode) -> Result<Response, Error> {
        Ok(match key {
            KeyCode::Char(x) if x.is_ascii_digit() => {
                self.state.push(x);
                Response::Ok
            }
            KeyCode::Backspace => {
                self.state.pop();
                if self.state.is_empty() {
                    Response::Pop
                } else {
                    Response::Ok
                }
            }
            KeyCode::Char('g' | 'G') => {
                model.scroll = self.count() as u16 - 1;
                model.scroll_by(0);
                Response::Pop
            }
            _ => {
                model.movement_count = self.count().min(i16::MAX as usize) as i16;
                Response::PopWith(key)
            }
        })
    }
}

#[derive(Default)]
struct SearchHandler {
    needle: String,
}
impl KeyPressHandler for SearchHandler {
    fn status<'a>(&'a self) -> Option<Line<'a>> {
        let mut line = Line::default();
        line.spans.push(Span::from("/").fg(Color::Indexed(148)));
        line.spans.push(Span::from(&self.needle));
        Some(line)
    }

    fn enter(&mut self, model: &mut Model) -> Result<(), Error> {
        model.cursor = Cursor::None;
        model.add_searches(None);
        Ok(())
    }

    fn handle(&mut self, model: &mut Model, key: KeyCode) -> Result<Response, Error> {
        Ok(match key {
            KeyCode::Char(x) => {
                self.needle.push(x);
                model.add_searches(Some(&self.needle));
                Response::Ok
            }
            KeyCode::Backspace if !self.needle.is_empty() => {
                self.needle.pop();
                model.add_searches(Some(&self.needle));
                Response::Ok
            }
            KeyCode::Backspace | KeyCode::Esc => Response::PopWith(key),
            KeyCode::Enter => {
                model.cursor = Cursor::Search(
                    crate::cursor::SearchState {
                        needle: std::mem::take(&mut self.needle),
                    },
                    None,
                );
                model.cursor_next();
                Response::Pop
            }
            _ => Response::Ignored,
        })
    }
}
