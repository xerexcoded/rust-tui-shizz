use chrono::prelude::*; // for handling date creation
use crossterm::{
    event::{self,Event as CEvent,KeyCode},
    terminal::{disable_raw_mode,enable_raw_mode},
}; // for terminal backend
use rand::{distributions::Alphanumeric,prelude::*};
use serde::{Deserialize,Serialize}; // for handling json 
use std::{fs, thread, usize};
use std::io;
use std::sync::mpsc;
use std::time::{Duration,Instant};
use thiserror::Error;
use tui:: {
    backend::CrosstermBackend,
    layout::{Alignment,Constraint,Direction,Layout},
    style::{Color,Modifier,Style},
    text::{Span,Spans},
    widgets::{
        Block,BorderType,Borders,Cell,List,ListItem,ListState,Paragraph,Row,Table,Tabs
    },
    Terminal,
} ;
const DB_PATH :&str="./data/db.json";


//handling internal error types , as we might run into some i/o errors
#[derive(Error,Debug)]
pub enum Error {
    #[error("error reading in DBfile: {0}")]
    ReadDBError(#[from] io::Error),
    #[error("error parsing the DB file: {0}")]
    ParseDBError(#[from] serde_json::Error),
}

// data structure for input events 
// input is either an user input 
// or a tick , tick_rate is defined to emit a tick if nothing happens 
enum Event<I> {
    Input(I),
    Tick,
}
#[derive(Serialize,Deserialize,Clone)]
struct Pet {  //defining data strcuture for what a pet should look like
    id: usize,
    name:String,
    category: String,
    age:usize,
    created_at:DateTime<Utc>,
}
#[derive(Clone, Copy,Debug)]
enum MenuItem {  // data strcuture for determining where we are in the app
    Home,Pets,
}
impl From<MenuItem> for usize {
    fn from(input:MenuItem) -> usize { //takes MenuItem as input and returns usize , thus enabling us mapping to currently selected tab

        match input {
            MenuItem::Home => 0,
            MenuItem::Pets =>1,
        }
    }
}


// enabling raw mode lets us elimainate the need of waiting for terminal input from user

fn main() -> Result<(),Box<dyn std::error::Error>> {
    enable_raw_mode().expect("can run in raw mode");

    //setting up mpsc for comminicating between input handler and rendering loop
    let (tx,rx) = mpsc::channel();
    let tick_rate=Duration::from_millis(50);// don't set too low as it will use more resources , but speed thrills :]!
    thread::spawn(move || {
        let mut last_tick =Instant::now();
        // this input is spawned in another thread as main thread is used to render the app and the
        // input loop doesn't block rendering
        loop { // input loop
            // calculate timeout(next tick)
            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));
            //event::poll to wait until timeout for an event 
            //and if there is one ,send it through our channel
            //with the key user pressed
            if event::poll(timeout).expect("poll works") {
                if let CEvent::Key(key) =event::read().expect("can you read events"){

                    //send key inputted by the user
                    tx.send(Event::Input(key)).expect("can send events");
                }
            }
            if last_tick.elapsed() >= tick_rate { //if time elapsed send tick
                if let Ok(_) = tx.send(Event::Tick){
                    last_tick=Instant::now();
                }
            }

        }
    });
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);// to get a new backend 
    let mut terminal =Terminal::new(backend)?;
    terminal.clear()?; //clear the terminal


    let menu_titles = vec!["Home","Pets","Add","Delete","Quit"];
    let mut active_menu_item =MenuItem::Home;//default tab
    let mut pet_list_state = ListState::default();
    pet_list_state.select(Some(0));
    loop { // loop call terminal.draw() at every iteration

        // draw function is given a closure 
        // which recieves a rect[layout primitive for rectangle in TUI]
        terminal.draw(|rect| {
            let size = rect.size();
            // Menu of length ->3
            //middle content of atleast 2 (free to expand)
            //Footer of 3
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints([Constraint::Length(3),Constraint::Min(2),Constraint::Length(3),].as_ref(),).split(size);
     
              // footer
            let copyright =Paragraph::new("mau-cli 2021 - no rights reserved")
                .style(Style::default().fg(Color::LightGreen))
                .alignment(Alignment::Center)
                .block(Block::default() //block defines where you can put the title and optional border around the content
                       .borders(Borders::ALL)
                       .style(Style::default().fg(Color::White))
                       .title("poppyRight")
                       .border_type(BorderType::Double),
                       );
            let menu = menu_titles
                .iter()
                .map(|t| {
                    let (first,rest) = t.split_at(1);//at 1st chracter of each string in the vec
                    Spans::from(vec![
                                Span::styled(first,
                                             Style::default()
                                             .fg(Color::LightGreen)
                                             .add_modifier(Modifier::UNDERLINED),
                                             ),
                                             Span::styled(rest,Style::default().fg(Color::LightRed)),
                    ])
                }).collect();
            let tabs =Tabs::new(menu)
                .select(active_menu_item.into())
                .block(Block::default().title("Menu").borders(Borders::ALL))
                .style(Style::default().fg(Color::White))
                .highlight_style(Style::default().fg(Color::LightMagenta))
                .divider(Span::raw("|"));
            rect.render_widget(tabs,chunks[0]);
            match active_menu_item {
                MenuItem::Home => rect.render_widget(render_home(), chunks[1]),
                MenuItem::Pets => {
                    let pets_chunks =Layout::default()
                        .direction(Direction::Horizontal)//to display pets page with 2 elements adjacent to each in horizontal orientation
                        .constraints([Constraint::Percentage(20),Constraint::Percentage(80)].as_ref(),).split(chunks[1]);//spliting only the middle chunk
                    let (left,right) = render_pets(&pet_list_state);
                    rect.render_stateful_widget(left, pets_chunks[0], &mut pet_list_state);
                  rect.render_widget(right,pets_chunks[1]); 

            }
            }
            rect.render_widget(copyright, chunks[2]);

        })?;
        // handling input
        // we always render the current state first 
        // and the react to new input on the recieving end of our channel
        match rx.recv()?{//matching to corresponding key
            Event::Input(event) => match event.code {
                KeyCode::Char('q') => {
                    disable_raw_mode()?;// for quitting
                    terminal.show_cursor()?;//return command prompt and break out of input loop
                    break;
                }
                // set respective routes
                KeyCode::Char('h') => active_menu_item =MenuItem::Home,
                KeyCode::Char(('p')) => active_menu_item = MenuItem::Pets,
                KeyCode::Char('a') => {
                       add_random_pet_to_db().expect("can add new random pet");
                }
                KeyCode::Char('d') => {
                     remove_pet_at_index(&mut pet_list_state).expect("can remove pet");
                }
                KeyCode::Down => {
                    if let Some(selected) = pet_list_state.selected() {
                        let amount_pets = read_db().expect("can you fetch pet list ").len();
                        if selected >= amount_pets -1 {
                            pet_list_state.select(Some(0));
                        }
                        else {
                            pet_list_state.select(Some(selected + 1));
                        }
                    }

                }
                KeyCode::Up => {
                    if let Some(selected)= pet_list_state.selected() {
                        let amount_pets = read_db().expect("can fetch pet list").len();
                        if selected >0 {
                            pet_list_state.select(Some(selected - 1));
                        }else {

                            pet_list_state.select(Some(amount_pets - 1));
                        }

                    }
                }
                _ => {}
                
            },
            Event::Tick => {}
        }
    }
    Ok(())
}

fn render_home<'a>() -> Paragraph<'a> {
    let home = Paragraph::new(vec![
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("Welcome")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("to")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::styled("mau-cli",Style::default().fg(Color::LightYellow),)]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("Press 'p' to acess pets , 'a' to add random new pets and 'd' to delete")]),

    ]).alignment(Alignment::Center)
      .block(
          Block::default()
              .borders(Borders::ALL)
              .style(Style::default().fg(Color::White))
              .title("Home")
              .border_type(BorderType::Double),
      );
    home
}
//List is stateful , comes like this by default in TUI
//render pets return a List and Table , both are TUI widgets
fn render_pets<'a>(pet_list_state: &ListState) -> (List<'a>,Table<'a>){
    let pets = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::LightCyan))
        .title("pets")
        .border_type(BorderType::Rounded);
    let pet_list = read_db().expect("can fetch pet list");//PARSES it into a vec
    let items: Vec<_> = pet_list
        .iter()
        .map(|pet| {
            ListItem::new(Spans::from(vec![Span::styled(
                pet.name.clone(),
                Style::default(),
            )]))
        }).collect();
    let selected_pet = pet_list
        .get(
            pet_list_state
                .selected()
                .expect("there is always a selected pet"),
        )
        .expect("exists")
        .clone();
    let list = List::new(items).block(pets).highlight_style(
        Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    );
    let pet_detail = Table::new(vec![Row::new(vec![
        Cell::from(Span::raw(selected_pet.id.to_string())),
        Cell::from(Span::raw(selected_pet.name)),
        Cell::from(Span::raw(selected_pet.category)),
        Cell::from(Span::raw(selected_pet.age.to_string())),
        Cell::from(Span::raw(selected_pet.created_at.to_string())),
    ])])
        .header(Row::new(vec![
            Cell::from(Span::styled(
                "ID",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Cell::from(Span::styled(
                "Name",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Cell::from(Span::styled(
                "Category",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Cell::from(Span::styled(
                "Age",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Cell::from(Span::styled(
                "Created_at",
                Style::default().add_modifier(Modifier::BOLD),
            )),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Green))
                .title("Detail")
                .border_type(BorderType::Rounded)
        )
        .widths(&[
            Constraint::Percentage(5),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(5),
            Constraint::Percentage(20),
        ]);//define width in percentage as it makes it responsive
    (list,pet_detail)
}
fn read_db() -> Result<Vec<Pet>,Error> {
    let db_content = fs::read_to_string(DB_PATH)?;
    let parsed: Vec<Pet> = serde_json::from_str(&db_content)?;
    Ok(parsed)
}
fn add_random_pet_to_db() -> Result<Vec<Pet>,Error>{
    let mut rng = rand::thread_rng();// to create random
    // not taking input from user yet
    let db_content = fs::read_to_string(DB_PATH)?;
    let mut parsed : Vec<Pet> = serde_json::from_str(&db_content)?;
    let catsdogs = match rng.gen_range(0,1) { // randomly assigned cat or dog
        0 => "cats",
        _ => "dogs"
    };
    let random_pet = Pet {
        id: rng.gen_range(0,9999999),
        name:rng.sample_iter(Alphanumeric).take(10).collect(),
        category : catsdogs.to_owned(),
        age:rng.gen_range(1,15),
        created_at :Utc::now(),
    };
    parsed.push(random_pet); //push on to the parsed db that was returned
    fs::write(DB_PATH,&serde_json::to_vec(&parsed)?)?;
    Ok(parsed)
}
fn remove_pet_at_index(pet_list_state:&mut ListState) -> Result<(),Error> {
    if let Some(selected) = pet_list_state.selected() {
        let db_content = fs::read_to_string(DB_PATH)?;
        let mut parsed : Vec<Pet> = serde_json::from_str(&db_content)?;
        parsed.remove(selected);
        fs::write(DB_PATH, &serde_json::to_vec(&parsed)?)?;
        pet_list_state.select(Some(selected - 1));//decreament the pet list 
    }
    Ok(())
}
