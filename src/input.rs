pub const GRID_SIZE: u32 = 26;
pub const ESCAPE_KEYCODE: u32 = 1;

pub fn precision_key_to_subcell(ch: char) -> Option<(u32, u32)> {
    match ch {
        'y' => Some((0, 0)),
        'u' => Some((1, 0)),
        'i' => Some((2, 0)),
        'o' => Some((3, 0)),
        'h' => Some((0, 1)),
        'j' => Some((1, 1)),
        'k' => Some((2, 1)),
        'l' => Some((3, 1)),
        _ => None,
    }
}

pub fn keycode_to_char(keycode: u32) -> Option<char> {
    match keycode {
        30 => Some('a'), 48 => Some('b'), 46 => Some('c'), 32 => Some('d'),
        18 => Some('e'), 33 => Some('f'), 34 => Some('g'), 35 => Some('h'),
        23 => Some('i'), 36 => Some('j'), 37 => Some('k'), 38 => Some('l'),
        50 => Some('m'), 49 => Some('n'), 24 => Some('o'), 25 => Some('p'),
        16 => Some('q'), 19 => Some('r'), 31 => Some('s'), 20 => Some('t'),
        22 => Some('u'), 47 => Some('v'), 17 => Some('w'), 45 => Some('x'),
        21 => Some('y'), 44 => Some('z'), _ => None,
    }
}

pub fn compute_cell_bounds(col: u32, row: u32, width: u32, height: u32) -> (u32, u32, u32, u32) {
    let x1 = (col * width) / GRID_SIZE;
    let x2 = if col == GRID_SIZE - 1 { width } else { ((col + 1) * width) / GRID_SIZE };
    let y1 = (row * height) / GRID_SIZE;
    let y2 = if row == GRID_SIZE - 1 { height } else { ((row + 1) * height) / GRID_SIZE };
    (x1, y1, x2, y2)
}

pub fn compute_precision_coordinate(
    first: char,
    second: char,
    precision_col: u32,
    precision_row: u32,
    width: u32,
    height: u32,
) -> (u32, u32) {
    let col = (second as u32) - ('a' as u32);
    let row = (first as u32) - ('a' as u32);
    let (cell_x1, cell_y1, cell_x2, cell_y2) = compute_cell_bounds(col, row, width, height);
    let cell_w = cell_x2 - cell_x1;
    let cell_h = cell_y2 - cell_y1;

    let sub_w = cell_w / 4;
    let sub_h = cell_h / 2;

    let x = cell_x1 + precision_col * sub_w + sub_w / 2;
    let y = cell_y1 + precision_row * sub_h + sub_h / 2;
    (x, y)
}

#[derive(Debug, Clone, Copy)]
pub enum InputState {
    Idle,
    WaitingSecond { first: char },
    WaitingThird { first: char, second: char },
    Done,
}
