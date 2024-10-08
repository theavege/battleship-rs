use std::{
  collections::{BTreeMap, BTreeSet},
  fmt::{self, Display},
};

use rand::{prelude::ThreadRng, seq::SliceRandom, Rng};
use structopt::clap::arg_enum;
use uuid::Uuid;

pub const ROWS: usize = 10;
pub const COLS: usize = 10;
const SHIP_SIZE: usize = 3;
const POS_ADDITION: [i32; 5] = [-2, -1, 0, 1, 2];
const ROTATIONS: [u16; 4] = [90, 180, 270, 360];

pub type Coordinate = (usize, usize);
type ShipShape = [[Status; SHIP_SIZE]; SHIP_SIZE];
type FiringResponse = BTreeMap<Coordinate, Status>;

arg_enum! {
    #[derive(Debug)]
    pub enum Rule {
      Default, // single shots
      Fury,    // not more than total number of ships alive
      Charge,  // not more than number of killed ships + 1
    }
}

arg_enum! {
    #[derive(PartialEq, Debug)]
    pub enum Difficulty {
        Easy, // computer generates random shots without previous ones
        Hard, // computer generates shots based on analysis of hit/miss  data
    }
}

pub struct Game {
  pub rule: Rule,
  difficulty: Difficulty,
  players: [Player; 2],
  winner: Option<usize>,
  turn: usize,
}

impl Game {
  pub fn new(rule: Rule, difficulty: Difficulty) -> Self {
    Self {
      turn: 0,
      winner: None,
      players: [Player::new(), Player::default()],
      rule,
      difficulty,
    }
  }

  fn player_by_turn_mut(&mut self, turn: usize) -> &mut Player {
    &mut self.players[turn]
  }

  fn generate_bot_firing_coordinates(&self) -> BTreeSet<Coordinate> {
    let mut rng = rand::thread_rng();

    let number_of_shots = match self.rule {
      Rule::Default => 1,
      Rule::Fury => self.computer().player_board().ships_alive().len(),
      Rule::Charge => {
        self.player().player_board().ships.len() - self.player().player_board().ships_alive().len()
          + 1
      }
    };

    let mut shots = BTreeSet::new();

    let previous_shots = self.computer().opponent_board().positions();

    let previous_shots = previous_shots
      .iter()
      .filter(|p| p.status != Status::Live && p.status != Status::Space)
      .collect::<Vec<_>>();

    let previous_hits = previous_shots
      .iter()
      .filter(|p| p.status == Status::Hit)
      .collect::<Vec<_>>();

    while shots.len() < number_of_shots {
      let shot = if self.difficulty == Difficulty::Easy {
        get_random_coordinate(&mut rng, 0)
      } else {
        // Generate cords based on previous hits, skip missed/hit slots and try slots near previous hits
        let shot = if previous_hits.is_empty() {
          get_random_coordinate(&mut rng, 0)
        } else {
          let coord = previous_hits
            .choose(&mut rng)
            .map_or((0, 0), |r| r.coordinate);

          let x_addition = POS_ADDITION.choose(&mut rng).unwrap_or(&0);
          let y_addition = POS_ADDITION.choose(&mut rng).unwrap_or(&0);
          let x = (coord.0 as i32) + x_addition;
          let y = (coord.1 as i32) + y_addition;
          let x = if x >= ROWS as i32 || x < 0 {
            coord.0
          } else {
            x as usize
          };
          let y = if y >= COLS as i32 || y < 0 {
            coord.1
          } else {
            y as usize
          };
          (x, y)
        };

        shot
      };

      if !previous_shots.iter().any(|p| p.coordinate == shot) {
        shots.insert(shot);
      }
    }

    shots
  }

  pub fn fire(&mut self, shots: &BTreeSet<Coordinate>, bot: bool) -> String {
    let player_index = self.turn;
    let opponent_index = 1 - player_index;
    let opponent = self.player_by_turn_mut(opponent_index);
    let opponent_board = opponent.player_board_mut();
    let (response, lost) = opponent_board.take_fire(shots);

    let player = self.player_by_turn_mut(player_index);
    let message = player.opponent_board_mut().update_status(response, bot);
    self.turn = opponent_index;
    if lost {
      self.winner = Some(player_index);
      if bot {
        "You lost 🙁".into()
      } else {
        "You won 🙌".into()
      }
    } else {
      message
    }
  }

  pub fn bot_fire(&mut self) -> String {
    let shots = self.generate_bot_firing_coordinates();
    self.fire(&shots, true)
  }

  pub fn is_user_turn(&self) -> bool {
    self.turn == 0
  }

  pub fn is_won(&self) -> bool {
    self.winner.is_some()
  }

  pub fn is_valid_rule(&self, existing_shots: usize) -> bool {
    match self.rule {
      Rule::Default => existing_shots < 1,
      Rule::Fury => existing_shots < self.player().player_board().ships_alive().len(),
      Rule::Charge => {
        existing_shots
          <= (self.computer().player_board().ships.len()
            - self.computer().player_board().ships_alive().len())
      }
    }
  }

  pub fn player(&self) -> &Player {
    &self.players[0]
  }

  pub fn computer(&self) -> &Player {
    &self.players[1]
  }
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum Status {
  Live,
  Miss,
  Hit,
  Kill,
  Space,
}

impl Display for Status {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let s = match *self {
      Status::Live => "🚀",
      Status::Miss => "❌",
      Status::Hit => "💥",
      Status::Kill => "💀",
      Status::Space => " ",
    };
    write!(f, "{}", s)
  }
}

#[derive(PartialEq, Clone)]
pub struct Player {
  is_bot: bool,
  boards: [Board; 2],
}

impl Player {
  fn new() -> Self {
    Self {
      is_bot: false,
      boards: [Board::new(true), Board::new(false)],
    }
  }

  pub fn player_board_mut(&mut self) -> &mut Board {
    &mut self.boards[0]
  }
  pub fn player_board(&self) -> &Board {
    &self.boards[0]
  }
  pub fn opponent_board_mut(&mut self) -> &mut Board {
    &mut self.boards[1]
  }
  pub fn opponent_board(&self) -> &Board {
    &self.boards[1]
  }
}

impl Default for Player {
  fn default() -> Self {
    Self {
      is_bot: true,
      ..Self::new()
    }
  }
}

#[derive(PartialEq, Clone)]
pub struct Board {
  pub positions: Vec<Vec<Position>>,
  ships: Vec<Ship>,
  firing_status: BTreeMap<String, String>,
}

impl Board {
  fn new(is_self: bool) -> Self {
    let mut rng = rand::thread_rng();
    // create empty positions
    let mut positions = (0..ROWS)
      .map(|r| (0..COLS).map(|c| Position::new((r, c))).collect::<Vec<_>>())
      .collect::<Vec<_>>();

    let ships = if is_self {
      let ship_types = ShipType::get_initial_ships();
      ship_types
        .iter()
        .map(|s_type| {
          let mut ship_placed = false;
          let mut ship = Ship::new(s_type.clone());
          // place ships on the board without overlap
          // doing this in a while loop is sub optimal as this is causing
          // infinite loop if number of ships are more than 4 currently
          while !ship_placed {
            let start_cords = get_random_coordinate(&mut rng, SHIP_SIZE);
            if !ship.is_overlapping(&positions, start_cords) {
              // draw ship on to board
              if ship.draw(&mut positions, start_cords) {
                ship_placed = true
              }
            } else {
              ship = Ship::new(s_type.clone());
            }
          }
          ship
        })
        .collect::<Vec<_>>()
    } else {
      vec![]
    };

    Self {
      ships,
      firing_status: BTreeMap::new(),
      positions,
    }
  }

  fn as_grid(&self) -> Vec<String> {
    self
      .positions
      .iter()
      .map(|row| {
        row
          .iter()
          .map(|c| c.to_string())
          .collect::<Vec<_>>()
          .join("")
      })
      .collect::<Vec<_>>()
  }

  fn ships_alive(&self) -> Vec<&Ship> {
    self.ships.iter().filter(|s| s.alive).collect::<Vec<_>>()
  }

  fn find_ship_mut(&mut self, id: String) -> Option<&mut Ship> {
    self.ships.iter_mut().find(|s| s.id == id)
  }

  fn find_ship(&self, id: String) -> Option<&Ship> {
    self.ships.iter().find(|s| s.id == id)
  }

  fn positions(&self) -> Vec<&Position> {
    self
      .positions
      .iter()
      .flat_map(|pr| pr.iter())
      .collect::<Vec<_>>()
  }

  fn pos_by_ship(&self, id: String) -> Vec<&Position> {
    self
      .positions()
      .into_iter()
      .filter(|pc| pc.ship_id.is_some() && pc.ship_id.clone().unwrap() == id)
      .collect::<Vec<_>>()
  }

  fn alive_pos_by_ship(&self, id: String) -> Vec<&Position> {
    self
      .pos_by_ship(id)
      .into_iter()
      .filter(|pc| pc.status == Status::Live)
      .collect::<Vec<_>>()
  }

  fn take_fire(&mut self, shots: &BTreeSet<Coordinate>) -> (FiringResponse, bool) {
    let mut response = BTreeMap::new();
    for shot in shots {
      let pos = self.positions[shot.0][shot.1].clone();
      let mut status = Status::Miss;
      if pos.status == Status::Live {
        status = Status::Hit;
        if let Some(id) = &pos.ship_id {
          if self.alive_pos_by_ship(id.clone()).len() <= 1 {
            let ship = self.find_ship_mut(id.clone());
            if let Some(ship) = ship {
              status = Status::Kill;
              ship.alive = false;
              let pos = self.pos_by_ship(id.clone());
              pos.iter().for_each(|p| {
                response.insert(p.coordinate, status);
              });
            }
          }
        }
      }
      if pos.status != Status::Hit && pos.status != Status::Kill {
        self.positions[shot.0][shot.1].status = status;
      }
      response.insert(*shot, status);
    }
    (response, self.ships_alive().is_empty())
  }

  fn update_status(&mut self, response: FiringResponse, bot: bool) -> String {
    let mut kill_count = 0;
    let mut hit_count = 0;
    let mut miss_count = 0;
    for (shot, status) in response {
      let pos = &mut self.positions[shot.0][shot.1];
      if pos.status == Status::Space || pos.status == Status::Live || status == Status::Kill {
        pos.status = status;
      }
      match status {
        Status::Miss => miss_count += 1,
        Status::Hit => hit_count += 1,
        Status::Kill => kill_count += 1,
        _ => {}
      }
    }
    let mut msg: Vec<String> = if bot {
      vec!["Computer have ".into()]
    } else {
      vec!["You have ".into()]
    };
    if kill_count > 0 {
      msg.push("sunk a ship.".to_string());
    } else {
      msg.push(format!("{} hit.", hit_count));
    }
    if miss_count > 0 {
      msg.push(format!(
        " {} missed {}.",
        if bot { "Computer" } else { "You" },
        miss_count
      ));
    }
    msg.join("")
  }

  pub fn find_position_and_ship(&self, coordinate: Coordinate) -> (&Position, Option<&Ship>) {
    let pos = &self.positions[coordinate.0][coordinate.1];
    if pos.ship_id.is_some() {
      (pos, self.find_ship(pos.ship_id.clone().unwrap()))
    } else {
      (pos, None)
    }
  }
}

impl Display for Board {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let s = self.as_grid().join("\n");
    write!(f, "{}", s)
  }
}

#[derive(PartialEq, Clone)]
pub struct Position {
  status: Status,
  coordinate: Coordinate,
  ship_id: Option<String>,
}

impl Position {
  fn new(coordinate: Coordinate) -> Self {
    Self {
      coordinate,
      status: Status::Space,
      ship_id: None,
    }
  }

  pub fn get_status(&self, ship: Option<&Ship>) -> Status {
    if ship.is_some() && !ship.unwrap().alive {
      Status::Kill
    } else {
      self.status
    }
  }
}

impl Display for Position {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{}", self.status)
  }
}

#[derive(PartialEq, Clone)]
pub struct Ship {
  id: String,
  rotation: u16,
  alive: bool,
  ship_type: ShipType,
}

impl Ship {
  fn new(ship_type: ShipType) -> Self {
    Self {
      id: Uuid::new_v4().to_string(),
      rotation: ROTATIONS.choose(&mut rand::thread_rng()).map_or(0, |r| *r),
      alive: true,
      ship_type,
    }
  }

  fn shape(&self) -> ShipShape {
    self.ship_type.get_shape(self.rotation)
  }

  fn is_overlapping(&self, positions: &[Vec<Position>], start_cord: Coordinate) -> bool {
    let mut ship_found = false;
    if !positions.is_empty() && !positions[0].is_empty() {
      let mut x = start_cord.0;
      for row in &self.shape() {
        let mut y = start_cord.1;
        for _ in row {
          if positions[x][y].status == Status::Live {
            ship_found = true;
          }
          y += 1;
        }
        x += 1;
      }
    }
    ship_found
  }

  fn draw(&self, positions: &mut [Vec<Position>], start_cord: Coordinate) -> bool {
    let mut ship_drawn = false;
    if !positions.is_empty() && !positions[0].is_empty() {
      let shape = self.shape();

      let mut x = start_cord.0;
      for row in &shape {
        let mut y = start_cord.1;
        for col in row {
          if Status::Live == *col {
            positions[x][y].status = Status::Live;
            positions[x][y].ship_id = Some(self.id.to_owned());
            ship_drawn = true
          }
          y += 1;
        }
        x += 1;
      }
    }
    ship_drawn
  }
}

#[derive(Clone, PartialEq)]
enum ShipType {
  X,
  V,
  H,
  I,
}

impl ShipType {
  fn get_shape(&self, rotation: u16) -> ShipShape {
    let shape = match *self {
      ShipType::X => [
        [Status::Live, Status::Space, Status::Live],
        [Status::Space, Status::Live, Status::Space],
        [Status::Live, Status::Space, Status::Live],
      ],
      ShipType::V => [
        [Status::Live, Status::Space, Status::Live],
        [Status::Live, Status::Space, Status::Live],
        [Status::Space, Status::Live, Status::Space],
      ],
      ShipType::H => [
        [Status::Live, Status::Space, Status::Live],
        [Status::Live, Status::Live, Status::Live],
        [Status::Live, Status::Space, Status::Live],
      ],
      ShipType::I => [
        [Status::Space, Status::Live, Status::Space],
        [Status::Space, Status::Live, Status::Space],
        [Status::Space, Status::Live, Status::Space],
      ],
    };

    match rotation {
      180 => reverse_cols_of_rows(transpose(shape)),
      270 => reverse_rows_of_cols(reverse_cols_of_rows(shape)),
      360 => reverse_rows_of_cols(transpose(shape)),
      _ => shape,
    }
  }

  fn get_initial_ships() -> [ShipType; 4] {
    [Self::X, Self::V, Self::H, Self::I]
  }
}

fn get_random_coordinate(rng: &mut ThreadRng, threshold: usize) -> Coordinate {
  (
    rng.gen_range(0..(ROWS - threshold)),
    rng.gen_range(0..(COLS - threshold)),
  )
}
/**
 * transpose a 2D char array.
 */
fn transpose(inp: ShipShape) -> ShipShape {
  if inp.is_empty() {
    //empty or unset array, nothing do to here
    return inp;
  }

  let mut out = inp;

  for (x, cols) in inp.iter().enumerate() {
    for (y, _) in cols.iter().enumerate() {
      out[y][x] = inp[x][y];
    }
  }
  out
}

/**
 * reverse columns of each rows in a 2d array.
 */
fn reverse_cols_of_rows(inp: ShipShape) -> ShipShape {
  if inp.is_empty() {
    //empty or unset array, nothing do to here
    return inp;
  }
  let mut out = inp;

  for (x, cols) in inp.iter().enumerate() {
    for (y, _) in cols.iter().enumerate() {
      out[x][cols.len() - y - 1] = inp[x][y];
    }
  }
  out
}

/**
 * reverse rows of each column in a 2d array.
 */
fn reverse_rows_of_cols(inp: ShipShape) -> ShipShape {
  if inp.is_empty() {
    //empty or unset array, nothing do to here
    return inp;
  }

  let mut out = inp;

  for (x, cols) in inp.iter().enumerate() {
    for (y, _) in cols.iter().enumerate() {
      out[inp.len() - x - 1][y] = inp[x][y];
    }
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn test_game_is_valid_rule() {
    let mut game = Game::new(Rule::Default, Difficulty::Easy);
    assert!(game.is_valid_rule(0));
    assert!(!game.is_valid_rule(1));

    game.rule = Rule::Fury;

    assert!(game.is_valid_rule(0));
    assert!(game.is_valid_rule(3));
    assert!(!game.is_valid_rule(4));

    game.rule = Rule::Charge;

    assert!(game.is_valid_rule(0));
    assert!(!game.is_valid_rule(1));
  }

  #[test]
  fn test_game_fire() {
    let mut game = Game::new(Rule::Default, Difficulty::Easy);

    let mut shots = BTreeSet::new();
    shots.insert((1, 1));
    shots.insert((3, 3));

    let msg = game.fire(&shots, false);

    assert!(!msg.is_empty());
    assert!(!game.is_user_turn());
    assert!(!game.winner.is_some());
  }

  #[test]
  fn test_game_generate_firing_coordinates() {
    let game = Game::new(Rule::Default, Difficulty::Easy);

    let shots = game.generate_bot_firing_coordinates();
    assert_eq!(shots.len(), 1);

    let game = Game::new(Rule::Charge, Difficulty::Easy);

    let shots = game.generate_bot_firing_coordinates();
    assert_eq!(shots.len(), 1);

    let game = Game::new(Rule::Fury, Difficulty::Easy);

    let shots = game.generate_bot_firing_coordinates();
    assert_eq!(shots.len(), 4);
  }

  #[test]
  fn test_get_random_coordinate() {
    let mut rng = rand::thread_rng();
    assert!(get_random_coordinate(&mut rng, SHIP_SIZE) < (ROWS, COLS));
  }

  #[test]
  fn test_reverse_rows_of_cols() {
    let ship = [
      [Status::Live, Status::Live, Status::Space],
      [Status::Space, Status::Live, Status::Space],
      [Status::Space, Status::Space, Status::Live],
    ];
    let expected = [
      [Status::Space, Status::Space, Status::Live],
      [Status::Space, Status::Live, Status::Space],
      [Status::Live, Status::Live, Status::Space],
    ];
    assert_eq!(reverse_rows_of_cols(ship), expected);
  }

  #[test]
  fn test_reverse_cols_of_rows() {
    let ship = [
      [Status::Live, Status::Live, Status::Space],
      [Status::Space, Status::Live, Status::Space],
      [Status::Space, Status::Space, Status::Space],
    ];
    let expected = [
      [Status::Space, Status::Live, Status::Live],
      [Status::Space, Status::Live, Status::Space],
      [Status::Space, Status::Space, Status::Space],
    ];
    assert_eq!(reverse_cols_of_rows(ship), expected);
  }

  #[test]
  fn test_transpose() {
    let ship = [
      [Status::Live, Status::Live, Status::Space],
      [Status::Space, Status::Live, Status::Space],
      [Status::Space, Status::Space, Status::Space],
    ];
    let expected = [
      [Status::Live, Status::Space, Status::Space],
      [Status::Live, Status::Live, Status::Space],
      [Status::Space, Status::Space, Status::Space],
    ];
    assert_eq!(transpose(ship), expected);
  }

  #[test]
  fn test_ship_type_get_shape() {
    let ship = ShipType::H;
    assert_eq!(
      ship.get_shape(90),
      [
        [Status::Live, Status::Space, Status::Live],
        [Status::Live, Status::Live, Status::Live],
        [Status::Live, Status::Space, Status::Live],
      ]
    );
    assert_eq!(
      ship.get_shape(180),
      [
        [Status::Live, Status::Live, Status::Live],
        [Status::Space, Status::Live, Status::Space],
        [Status::Live, Status::Live, Status::Live],
      ]
    );
    let ship = ShipType::V;
    assert_eq!(
      ship.get_shape(270),
      [
        [Status::Space, Status::Live, Status::Space],
        [Status::Live, Status::Space, Status::Live],
        [Status::Live, Status::Space, Status::Live],
      ]
    );
    assert_eq!(
      ship.get_shape(360),
      [
        [Status::Live, Status::Live, Status::Space],
        [Status::Space, Status::Space, Status::Live],
        [Status::Live, Status::Live, Status::Space],
      ]
    );
  }

  #[test]
  fn test_ship_is_overlapping() {
    let ship = Ship::new(ShipType::H);

    assert!(!ship.is_overlapping(&[], (0, 0)));
    assert!(!ship.is_overlapping(&[vec![]], (0, 0)));

    let mut positions = (0..ROWS)
      .map(|r| (0..COLS).map(|c| Position::new((r, c))).collect::<Vec<_>>())
      .collect::<Vec<_>>();
    // should pass as there is no overlap in default
    assert!(!ship.is_overlapping(&positions, (0, 0)));

    positions[1][5] = Position {
      coordinate: (1, 5),
      ship_id: Some("123".into()),
      status: Status::Live,
    };
    // should fail when there is overlap
    assert!(ship.is_overlapping(&positions, (1, 5)));
  }

  #[test]
  fn test_ship_draw() {
    let ship = Ship {
      id: "123".into(),
      rotation: 90,
      alive: true,
      ship_type: ShipType::H,
    };
    let mut positions = (0..ROWS)
      .map(|r| (0..COLS).map(|c| Position::new((r, c))).collect::<Vec<_>>())
      .collect::<Vec<_>>();
    assert!(ship.draw(&mut positions, (5, 5)));
    let p = positions
      .iter()
      .map(|row| {
        row
          .iter()
          .map(|c| c.to_string())
          .collect::<Vec<_>>()
          .join("")
      })
      .collect::<Vec<_>>()
      .join("\n");
    assert_eq!(p, "          \n          \n          \n          \n          \n     🚀 🚀  \n     🚀🚀🚀  \n     🚀 🚀  \n          \n          ");
    assert!(ship.is_overlapping(&positions, (5, 5)));
  }

  #[test]
  fn test_board_new() {
    let opponent_board = Board::new(false);

    // should be empty board initially
    assert_eq!(opponent_board.to_string(), "          \n          \n          \n          \n          \n          \n          \n          \n          \n          ");

    let my_board = Board::new(true);

    // should be empty board initially
    assert_eq!(my_board.ships.len(), 4);
    assert_eq!(my_board.positions.len(), ROWS);
    // check if all ships are placed on the board
    my_board.ships.iter().for_each(|it| {
      let found = my_board
        .positions
        .iter()
        .flat_map(|pr| pr.iter())
        .filter(|pc| pc.ship_id.is_some() && pc.ship_id.clone().unwrap() == it.id)
        .collect::<Vec<_>>();
      match it.ship_type {
        ShipType::X => assert!(found.len() == 5, "ship X not placed!"),
        ShipType::V => assert!(found.len() == 5, "ship V not placed!"),
        ShipType::H => assert!(found.len() == 7, "ship H not placed!"),
        ShipType::I => assert!(found.len() == 3, "ship I not placed!"),
      }
    })
  }

  #[test]
  fn test_board_take_fire() {
    let mut board = Board::new(true);

    board.positions[1][1].status = Status::Space;
    board.positions[3][3].status = Status::Live;

    let mut shots = BTreeSet::new();
    shots.insert((1, 1));
    shots.insert((3, 3));

    let (res, lost) = board.take_fire(&shots);
    assert_eq!(res.get(&(1, 1)).unwrap(), &Status::Miss);
    assert_eq!(res.get(&(3, 3)).unwrap(), &Status::Hit);
    assert!(!lost);

    let mut board = Board::new(true);

    // set a ship as hit except for one position
    let ship_id = board.ships[0].id.clone();
    let mut pos = board
      .positions
      .iter_mut()
      .flat_map(|pr| pr.iter_mut())
      .filter(|pc| pc.ship_id.is_some() && pc.ship_id.clone().unwrap() == ship_id)
      .collect::<Vec<_>>();

    pos.iter_mut().skip(1).for_each(|p| p.status = Status::Hit);
    let c = pos.iter().take(1).map(|p| p.coordinate).collect::<Vec<_>>();

    let mut shots = BTreeSet::new();
    shots.insert(c[0]);

    let (res, lost) = board.take_fire(&shots);
    assert_eq!(res.get(&c[0]).unwrap(), &Status::Kill);
    assert!(!lost);
  }

  #[test]
  fn test_board_update_status() {
    let mut board = Board::new(false);

    let mut res = BTreeMap::new();
    res.insert((1, 1), Status::Miss);
    res.insert((3, 3), Status::Hit);
    res.insert((0, 2), Status::Kill);

    let message = board.update_status(res, false);
    assert_eq!(message, "You have sunk a ship. You missed 1.");

    let mut res = BTreeMap::new();
    res.insert((3, 3), Status::Hit);
    res.insert((0, 2), Status::Hit);

    let message = board.update_status(res.clone(), false);
    assert_eq!(message, "You have 2 hit.");
    let message = board.update_status(res, true);
    assert_eq!(message, "Computer have 2 hit.");
  }
}
