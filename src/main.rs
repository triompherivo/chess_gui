use iced::{
    alignment, executor, font, Alignment, Application, Command, Element, Length,
    Settings, Theme, Color,
    widget::{Button, Column, Container, Row, Text}
};
use chess::{Board, ChessMove, Color as ChessColor, File, Game, GameResult, Piece, Rank, Square};
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;
use tokio::process::Command as AsyncCommand;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

struct UciMove(pub ChessMove);

impl fmt::Display for UciMove {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

fn main() -> iced::Result {
    ChessApp::run(Settings::default())
}

struct ChessApp {
    game: Game,
    selected_square: Option<Square>,
    stockfish_path: PathBuf,
    current_turn: ChessColor,
    status: String,
    engine_evaluation: String,
    principal_variation: Vec<ChessMove>,
}

#[derive(Debug, Clone)]
enum Message {
    SquareSelected(Square),
    EngineMove((ChessMove, String, Vec<ChessMove>)),
    NewGame,
}

impl Application for ChessApp {
    type Message = Message;
    type Theme = Theme;
    type Executor = executor::Default;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let game = Game::new();
        let stockfish_path = PathBuf::from("/usr/local/bin/stockfish");
        
        (
            Self {
                game,
                selected_square: None,
                stockfish_path,
                current_turn: ChessColor::White,
                status: "White's turn".to_string(),
                engine_evaluation: String::new(),
                principal_variation: Vec::new(),
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("Rust Chess - Stockfish")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        if self.game.result().is_some() {
            return Command::none();
        }

        match message {
            Message::SquareSelected(square) => {
                if self.current_turn == ChessColor::White {
                    if let Some(selected) = self.selected_square {
                        let mv = ChessMove::new(selected, square, None);
                        
                        if self.game.current_position().legal(mv) {
                            let mut new_game = self.game.clone();
                            if new_game.make_move(mv) {
                                self.game = new_game;
                                self.current_turn = ChessColor::Black;
                                self.status = "Stockfish is thinking...".to_string();
                                self.selected_square = None;
                                return get_stockfish_move(
                                    self.stockfish_path.clone(),
                                    self.game.clone()
                                );
                            }
                        }
                    }
                    self.selected_square = Some(square);
                }
                Command::none()
            }
            Message::EngineMove((mv, eval, pv)) => {
                let mut new_game = self.game.clone();
                if new_game.make_move(mv) {
                    self.game = new_game;
                    self.current_turn = ChessColor::White;
                    self.status = "White's turn".to_string();
                    self.engine_evaluation = eval;
                    self.principal_variation = pv;
                }
                Command::none()
            }
            Message::NewGame => {
                self.game = Game::new();
                self.current_turn = ChessColor::White;
                self.selected_square = None;
                self.status = "New game - White's turn".to_string();
                self.engine_evaluation.clear();
                self.principal_variation.clear();
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let board = self.game.current_position();
        let status = match self.game.result() {
            Some(GameResult::WhiteCheckmates) => "White wins by checkmate!",
            Some(GameResult::BlackCheckmates) => "Black wins by checkmate!",
            Some(GameResult::Stalemate) => "Draw by stalemate",
            Some(GameResult::DrawAccepted) => "Draw accepted",
            Some(GameResult::WhiteResigns) => "White resigns. Black wins!",
            Some(GameResult::BlackResigns) => "Black resigns. White wins!",
            Some(GameResult::DrawDeclared) => "Draw declared",
            None => &self.status,
        };

        let mut rows = Column::new().spacing(5);
        
        // Proper board orientation (White at bottom)
        for rank in (0..8).rev() {
            let mut row = Row::new().spacing(5);
            
            for file in 0..8 {
                let square = Square::make_square(
                    Rank::from_index(rank),
                    File::from_index(file)
                );
                let piece = board.piece_on(square);
                let color = board.color_on(square).unwrap_or(ChessColor::White);
                let is_light_square = (file + rank) % 2 == 0;
                
                // Square colors
                let button_color = if self.selected_square == Some(square) {
                    Color::from_rgb(0.7, 0.7, 0.0) // Yellow for selected
                } else if is_light_square {
                    Color::from_rgb(0.73, 0.73, 0.73) // Light squares
                } else {
                    Color::from_rgb(0.25, 0.25, 0.25) // Dark squares
                };

                // Piece colors with clear contrast
                let (text_color, symbol) = match color {
                    ChessColor::White => (
                        Color::from_rgb(0.95, 0.95, 0.95), // Bright white
                        white_piece_symbol(piece)
                    ),
                    ChessColor::Black => (
                        Color::from_rgb(0.1, 0.1, 0.1),    // Dark black
                        black_piece_symbol(piece)
                    ),
                };

                let button = Button::new(
                    Text::new(symbol)
                        .size(40)
                        .font(font::Font::with_name("Arial Unicode MS"))
                        .horizontal_alignment(alignment::Horizontal::Center)
                        .vertical_alignment(alignment::Vertical::Center)
                        .style(text_color)
                )
                .width(70)
                .height(70)
                .style(iced::theme::Button::Custom(Box::new(ButtonStyle(button_color))))
                .on_press(Message::SquareSelected(square));
                
                row = row.push(button);
            }
            rows = rows.push(row);
        }

        let analysis = Column::new()
            .spacing(10)
            .push(Text::new(status).size(18))
            .push(Text::new(&self.engine_evaluation).size(16))
            .push(Text::new("Principal Variation:").size(16))
            .push(
                Text::new(
                    self.principal_variation.iter()
                        .take(5)
                        .map(|mv| UciMove(*mv).to_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                ).size(14)
            );

        let controls = Column::new()
            .spacing(20)
            .push(Button::new("New Game").on_press(Message::NewGame))
            .push(analysis);

        Container::new(
            Row::new()
                .push(rows)
                .push(controls)
                .spacing(30)
                .align_items(Alignment::Center)
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x()
        .center_y()
        .padding(30)
        .into()
    }
}

struct ButtonStyle(Color);
impl iced::widget::button::StyleSheet for ButtonStyle {
    type Style = iced::Theme;

    fn active(&self, _style: &Self::Style) -> iced::widget::button::Appearance {
        iced::widget::button::Appearance {
            background: Some(self.0.into()),
            border: iced::Border {
                radius: 3.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            text_color: Color::TRANSPARENT,
            ..Default::default()
        }
    }
}

fn white_piece_symbol(piece: Option<Piece>) -> String {
    match piece {
        Some(Piece::King) => '♔',
        Some(Piece::Queen) => '♕',
        Some(Piece::Rook) => '♖',
        Some(Piece::Bishop) => '♗',
        Some(Piece::Knight) => '♘',
        Some(Piece::Pawn) => '♙',
        None => ' ',
    }.to_string()
}

fn black_piece_symbol(piece: Option<Piece>) -> String {
    match piece {
        Some(Piece::King) => '♚',
        Some(Piece::Queen) => '♛',
        Some(Piece::Rook) => '♜',
        Some(Piece::Bishop) => '♝',
        Some(Piece::Knight) => '♞',
        Some(Piece::Pawn) => '♟',
        None => ' ',
    }.to_string()
}

fn get_stockfish_move(path: PathBuf, game: Game) -> Command<Message> {
    Command::perform(
        async move {
            let mut stockfish = AsyncCommand::new(&path)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .spawn()
                .expect("Failed to start Stockfish");

            let fen = game.current_position().to_string();
            let commands = format!(
                "uci\nisready\nucinewgame\nposition fen {}\n\
                 setoption name Skill Level value 20\n\
                 setoption name Contempt value 100\n\
                 setoption name UCI_LimitStrength value false\n\
                 go movetime 5000\n",
                fen
            );
            if let Some(mut stdin) = stockfish.stdin.take() {
                stdin.write_all(commands.as_bytes()).await.expect("Write failed");
                stdin.flush().await.expect("Flush failed");
            }

            let mut output = String::new();
            let mut evaluation = String::new();
            let mut pv = Vec::new();
            let mut best_move = None;

            if let Some(mut stdout) = stockfish.stdout.take() {
                let mut buf = [0u8; 1024];
                loop {
                    let n = stdout.read(&mut buf).await.expect("Read failed");
                    if n == 0 { break; }
                    output.push_str(&String::from_utf8_lossy(&buf[..n]));
                    
                    for line in output.lines() {
                        if line.starts_with("info") {
                            if let Some(score_idx) = line.find("score cp") {
                                let parts: Vec<&str> = line.split_whitespace().collect();
                                if let Some(cp_idx) = parts.iter().position(|&s| s == "cp") {
                                    if let Some(cp) = parts.get(cp_idx + 1) {
                                        evaluation = format!("Evaluation: {}{}", 
                                            if parts.contains(&"lowerbound") { "≥" } 
                                            else if parts.contains(&"upperbound") { "≤" } 
                                            else { "" },
                                            cp
                                        );
                                    }
                                }
                            }
                            if let Some(pv_idx) = line.find("pv") {
                                pv = line[pv_idx+3..]
                                    .split_whitespace()
                                    .filter_map(|m| ChessMove::from_str(m).ok())
                                    .collect();
                            }
                        }
                        if line.starts_with("bestmove") {
                            best_move = line.split_whitespace()
                                .nth(1)
                                .and_then(|m| ChessMove::from_str(m).ok());
                            break;
                        }
                    }
                    
                    if best_move.is_some() {
                        break;
                    }
                }
            }

            (
                best_move.expect("No best move found"),
                evaluation,
                pv
            )
        },
        Message::EngineMove
    )
}