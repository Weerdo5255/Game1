use macroquad::prelude::*;

fn conf() -> Conf {
    Conf {
        window_title: "QUIETLY, EVERYTHING".to_owned(),
        window_width: 1280,
        window_height: 720,
        ..Default::default()
    }
}

#[macroquad::main(conf)]
async fn main() {
    loop {
        clear_background(BLACK);
        next_frame().await
    }
}
