use crate::renderer::Renderer;

// Uygulama olaylari - koordinatlar GOVDE-YEREL (0,0 = govde sol ust)
pub enum AppEvent {
    Click { x: i32, y: i32 }, // basma ANI
    Drag { x: i32, y: i32 },  // basili surukleme
    Key { ch: char },
    RClick { x: i32, y: i32 },
}

// Her Rusty uygulamasinin sozlesmesi. App Manager pencereyi acar,
// govde alanini verir; uygulama SADECE kendi alanina cizer.
pub trait App {
    fn title(&self) -> &'static str;
    fn draw(&mut self, r: &Renderer, x: usize, y: usize, w: usize, h: usize);
    fn on_event(&mut self, ev: &AppEvent) -> bool; // true = yeniden ciz
}