use crate::renderer::Renderer;

// app cordinates local
pub enum AppEvent {
    Click { x: i32, y: i32 }, // click
    Drag { x: i32, y: i32 },  // drag
    Key { ch: char },
    RClick { x: i32, y: i32 },
}

pub trait App {
    fn title(&self) -> &'static str;
    fn draw(&mut self, r: &Renderer, x: usize, y: usize, w: usize, h: usize);
    fn on_event(&mut self, ev: &AppEvent) -> bool; // true = draw again
}
