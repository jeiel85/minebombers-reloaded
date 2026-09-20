use crate::context::{Animation, ApplicationContext};
use crate::effects::SoundEffect;
use crate::error::ApplicationError::SdlError;
use crate::glyphs::{AnimationPhase, Border, Digging, Glyph};
use crate::highscore::{Highscores, Score};
use crate::keys::Key;
use crate::menu::shop::ShopResult;
use crate::options::WinCondition;
use crate::roster::PlayersRoster;
use crate::settings::GameSettings;
use crate::world::actor::{ActorComponent, ActorKind};
use crate::world::map::{LevelInfo, LevelMap, MapValue, DIRT_BORDER_BITMAP, MAP_COLS, MAP_ROWS};
use crate::world::player::{GlyphCheat, PlayerComponent};
use crate::world::position::{Cursor, Direction};
use crate::world::{Maps, SplatterKind, Update, World};
use crate::Application;
use rand::prelude::*;
use sdl2::event::Event;
use sdl2::keyboard::Scancode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::WindowCanvas;
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

const CAMPAIGN_ROUNDS: u16 = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundEnd {
  /// Round end (all gold collected in multiplayer, all opponents are dead, etc)
  Round,
  /// Failed round: playing single player and died
  Failed,
  /// Abort match directly back to main menu
  AbortToMenu,
}

impl Application<'_> {
  /// Play game, starting from player selection
  pub fn play_game(&self, ctx: &mut ApplicationContext, settings: &GameSettings) -> Result<(), anyhow::Error> {
    sdl2::mixer::Music::halt();
    let campaign_mode = settings.options.players == 1 || settings.options.campaign_mode;
    let selected = self.players_select_menu(ctx, settings.options.players)?;
    if selected.is_empty() {
      return Ok(());
    }

    let mut players = Vec::with_capacity(selected.len());
    let mut players_to_roster = Vec::with_capacity(selected.len());
    for (idx, selected) in selected.into_iter().enumerate() {
      players.push(PlayerComponent::new(
        selected.name,
        settings.keys.keys[idx],
        &settings.options,
        selected.is_bot,
        selected.bot_difficulty,
      ));
      players_to_roster.push(selected.roster_index);
    }

    if campaign_mode {
      // In single player, we start with 250 for each player
      players[0].cash = 250 * u32::from(settings.options.players);
      players[0].lives = 3;
    }

    let mut round = 0;
    while (!campaign_mode && round < settings.options.rounds)
      || (campaign_mode && players[0].lives > 0 && round < CAMPAIGN_ROUNDS)
    {
      ctx.with_render_context(|canvas| {
        canvas.set_draw_color(Color::BLACK);
        canvas.clear();
        let color = self.main_menu.palette[1];
        self
          .font
          .render(canvas, 220, 200, color, "Creating level...please wait")?;
        Ok(())
      })?;

      // Select a level to play
      ctx.animate(Animation::FadeUp, 7)?;
      let slot;
      let level = if campaign_mode {
        slot = LevelMap::prepare_campaign_level(ctx.game_dir(), round)?;
        &slot
      } else {
        settings
          .levels
          .get(usize::from(round))
          .map(Rc::as_ref)
          .unwrap_or(&LevelInfo::Random)
      };
      ctx.animate(Animation::FadeDown, 7)?;
      let result = self.play_round(ctx, &mut players, round, level, settings, campaign_mode)?;
      if campaign_mode && players[0].lives == 0 {
        // End of game: out of lives!
        break;
      }
      match result {
        RoundEnd::AbortToMenu => {
          sdl2::mixer::Music::halt();
          return Ok(());
        }
        RoundEnd::Failed => {
          // Keep playing the same round!
        }
        RoundEnd::Round => {
          round += 1;
        }
      }
    }

    if campaign_mode {
      self.campaign_end(ctx, round == CAMPAIGN_ROUNDS)?;
      self.hall_of_fame(ctx, round as u8, &players[0])?;
    } else {
      self.multi_player_end(ctx, &players, settings.options.win)?;
      update_player_stats(ctx.user_dir(), &mut players, &players_to_roster, settings.options.win)?;
    }
    Ok(())
  }

  /// Show ending screen of a campaign game
  fn campaign_end(&self, ctx: &mut ApplicationContext, win: bool) -> Result<(), anyhow::Error> {
    let texture = if win {
      &self.game_win.texture
    } else {
      &self.game_over.texture
    };
    ctx.with_render_context(|canvas| {
      canvas.copy(texture, None, None).map_err(SdlError)?;
      Ok(())
    })?;
    ctx.animate(Animation::FadeUp, 7)?;
    if win {
      self
        .effects
        .play(SoundEffect::Applause, 11000, Cursor::new(0, MAP_COLS / 2))?;
    }
    ctx.wait_key_pressed();
    ctx.animate(Animation::FadeDown, 7)?;
    Ok(())
  }

  /// Show hall of fame for a single player game
  fn hall_of_fame(
    &self,
    ctx: &mut ApplicationContext,
    rounds: u8,
    player: &PlayerComponent,
  ) -> Result<(), anyhow::Error> {
    let mut scores = Highscores::load(ctx.user_dir())?;
    let pos = scores
      .scores
      .binary_search_by(|score| {
        rounds
          .cmp(score.as_ref().map_or(&0, |s| &s.level))
          .then_with(|| player.cash.cmp(score.as_ref().map_or(&0, |s| &s.cash)))
      })
      .unwrap_or_else(|pos| pos);
    if pos < scores.scores.len() {
      // Drop the last element, replace it with the new score
      scores.scores[pos..].rotate_right(1);
      scores.scores[pos] = Some(Score {
        name: player.stats.name.to_owned(),
        level: rounds,
        cash: player.cash,
      });
      scores.save(ctx.user_dir())?;
    }

    // FIXME: implement rendering!
    ctx.with_render_context(|canvas| {
      canvas.copy(&self.halloffa.texture, None, None).map_err(SdlError)?;
      let color = self.halloffa.palette[1];
      for (idx, score) in scores.scores.iter().enumerate() {
        if let Some(score) = score {
          let text = format!(
            "{:<2}    {:<20}Level {:<2} Money {}",
            idx + 1,
            score.name,
            score.level,
            score.cash
          );
          self.font.render(canvas, 127, 10 * (idx as i32) + 179, color, &text)?;
        }
      }
      Ok(())
    })?;
    ctx.animate(Animation::FadeUp, 7)?;
    ctx.wait_key_pressed();
    ctx.animate(Animation::FadeDown, 7)?;
    Ok(())
  }

  /// Show end screen for a multiplayer game
  fn multi_player_end(
    &self,
    ctx: &mut ApplicationContext,
    players: &[PlayerComponent],
    win: WinCondition,
  ) -> Result<(), anyhow::Error> {
    ctx.with_render_context(|canvas| {
      canvas.copy(&self.r#final.texture, None, None).map_err(SdlError)?;
      for idx in 0..players.len() {
        let score = compute_score(players, idx, win);
        let avatars = &self.avatars[idx];
        let dest = Rect::new(32 + 150 * (idx as i32), 95, 132, 218);
        let texture = match score {
          PlayerWin::Win => &avatars.win.texture,
          PlayerWin::Lose => &avatars.lose.texture,
          PlayerWin::Draw => &avatars.draw.texture,
        };
        canvas.copy(texture, None, dest).map_err(SdlError)?;
        let color = self.r#final.palette[1];
        self
          .font
          .render(canvas, 36 + 150 * (idx as i32), 330, color, &players[idx].stats.name)?;
        self.font.render(
          canvas,
          36 + 150 * (idx as i32),
          362,
          color,
          &players[idx].rounds_win.to_string(),
        )?;
        self.font.render(
          canvas,
          36 + 150 * (idx as i32),
          346,
          color,
          &players[idx].cash.to_string(),
        )?;
      }
      Ok(())
    })?;
    ctx.animate(Animation::FadeUp, 7)?;
    self
      .effects
      .play(SoundEffect::Applause, 11000, Cursor::new(0, MAP_COLS / 2))?;
    ctx.wait_key_pressed();
    ctx.animate(Animation::FadeDown, 7)?;

    // FIXME: save stats back!
    Ok(())
  }

  /// Play a single game round
  fn play_round(
    &self,
    ctx: &mut ApplicationContext,
    players: &mut [PlayerComponent],
    round: u16,
    level: &LevelInfo,
    settings: &GameSettings,
    campaign_mode: bool,
  ) -> Result<RoundEnd, anyhow::Error> {
    // Note: in original game, single player is always played dark. However, in this
    // re-implementation I'm relaxing this as I never had patience to play through all 15 levels
    // with darkness 😅
    let darkness = settings.options.darkness; // || players.len() == 1;
    let level = match level {
      LevelInfo::Random => {
        let mut level = LevelMap::random_map(settings.options.treasures);
        level.generate_entrances(settings.options.players);
        level
      }
      LevelInfo::File { map, .. } => map.clone(),
    };

    // Play shop music
    self.music2.play(-1).map_err(SdlError)?;
    sdl2::mixer::Music::set_pos(464.8).map_err(SdlError)?;

    let mut shared_cash = if campaign_mode { Some(players[0].cash) } else { None };
    let mut it = players.iter_mut();
    while let Some(right) = it.next() {
      let left = it.next();
      let total_rounds = if campaign_mode { 15 } else { settings.options.rounds };
      let remaining = total_rounds - round;
      let preview_map = if darkness { None } else { Some(&level) };
      if self.shop(
        ctx,
        remaining,
        &settings.options,
        preview_map,
        &mut shared_cash,
        left,
        right,
      )? == ShopResult::ExitGame
      {
        sdl2::mixer::Music::halt();
        return Ok(RoundEnd::AbortToMenu);
      }
    }

    if let Some(cash) = shared_cash {
      players[0].cash = cash;
    }
    let is_gold_rush = settings.options.win == WinCondition::GoldRush;
    let is_survival = settings.options.win == WinCondition::Survival;
    let mut world = World::create(level, players, darkness, settings.options.bomb_damage, campaign_mode)
      .with_gold_rush_mode(is_gold_rush)
      .with_survival_mode(is_survival, (round + 1) as u16);

    sdl2::mixer::Music::halt();
    // FIXME: start playing random music from the level music; also, don't play shop music?
    self.music2.play(-1).map_err(SdlError)?;
    let mut music_on = true;

    ctx.with_render_context(|canvas| {
      self.render_game_screen(canvas, &world)?;
      Ok(())
    })?;
    ctx.animate(Animation::FadeUp, 7)?;

    let start = Instant::now();
    let mut paused_time = Duration::from_secs(0);
    let mut speed_multiplier: f32 = ctx.config.gameplay.default_speed;
    // Calculate base tick duration from options.speed (0 is 100% speed, 8 is 76% speed, 33 is 1% speed)
    let speed_pct = (100.0 - 3.0 * (settings.options.speed as f32)).max(10.0);
    let base_tick_ms = 20.0 * 100.0 / speed_pct;

    let mut active_explosions: Vec<(u16, u16, u8)> = Vec::new();

    let exit_reason = 'round: loop {
      world.tick();

      // Handle player commands
      if world.round_counter % 2 == 0 {
        // FIXME: in original game, command has slight delay on facing direction
        //  However, facing seems to be only used when holding still, so doesn't really matter much.

        let mut paused = false;
        let mut display_toggle_fullscreen = false;
        let mut display_set_scale: Option<u32> = None;
        let mut display_toggle_aspect = false;
        let mut toggle_crt = false;
        let mut toggle_dynamic_lighting = false;

        for event in ctx.poll_events() {
          match event {
            Event::Quit { .. } => {
              std::process::exit(0);
            }
            Event::ControllerButtonDown { button, .. } => {
              use sdl2::controller::Button;
              match button {
                Button::Back => break 'round RoundEnd::AbortToMenu,
                Button::Start => {
                  paused = true;
                }
                _ => {}
              }
            }
            Event::KeyDown {
              scancode: Some(scancode),
              keymod,
              ..
            } => {
              match scancode {
                Scancode::Escape | Scancode::F10 => break 'round RoundEnd::AbortToMenu,
                // Speed control hotkeys
                Scancode::LeftBracket | Scancode::Minus | Scancode::KpMinus => {
                  speed_multiplier = (speed_multiplier - 0.25).max(0.25);
                  println!("[MINEBOMBERS] Game speed: {:.2}x", speed_multiplier);
                }
                Scancode::RightBracket | Scancode::Equals | Scancode::KpPlus => {
                  speed_multiplier = (speed_multiplier + 0.25).min(3.0);
                  println!("[MINEBOMBERS] Game speed: {:.2}x", speed_multiplier);
                }
                Scancode::Backspace | Scancode::Num0 | Scancode::Kp0 => {
                  speed_multiplier = 1.0;
                  println!("[MINEBOMBERS] Game speed reset: 1.00x");
                }
                // Display mode hotkeys
                Scancode::F11 => {
                  display_toggle_fullscreen = true;
                }
                Scancode::F1 => {
                  display_set_scale = Some(1);
                }
                Scancode::F2 => {
                  display_set_scale = Some(2);
                }
                Scancode::F3 => {
                  display_set_scale = Some(3);
                }
                Scancode::F4 => {
                  display_toggle_aspect = true;
                }
                Scancode::F6 => {
                  toggle_crt = true;
                }
                Scancode::F7 => {
                  toggle_dynamic_lighting = true;
                }
                Scancode::Return
                  if keymod.intersects(sdl2::keyboard::Mod::LALTMOD | sdl2::keyboard::Mod::RALTMOD) =>
                {
                  display_toggle_fullscreen = true;
                }
                // FIXME: some better scancode?
                Scancode::Pause => {
                  paused = true;
                }
                Scancode::F5 => {
                  if music_on {
                    sdl2::mixer::Music::pause();
                  } else {
                    sdl2::mixer::Music::resume();
                  }
                  music_on = !music_on;
                }
                _ => {}
              }

              for player in 0..world.players.len() {
                // Bots are driven by AI, skip human keyboard mapping
                if world.players[player].is_bot {
                  continue;
                }
                let keys = world.players[player].keys;
                for key in Key::all_keys() {
                  if keys[key] == Some(scancode) {
                    world.player_action(player, key);
                  }
                }
              }
            }
            _ => {}
          }
        }

        // Query gamepad actions for each human player
        for player in 0..world.players.len() {
          if world.players[player].is_bot {
            continue;
          }
          for key in ctx.gamepad.get_player_actions(player) {
            world.player_action(player, key);
          }
        }
        if display_toggle_fullscreen {
          let _ = ctx.toggle_fullscreen();
        }
        if let Some(s) = display_set_scale {
          let _ = ctx.set_window_scale(s);
        }
        if display_toggle_aspect {
          let _ = ctx.toggle_aspect_ratio();
        }
        if toggle_crt {
          let _ = ctx.toggle_crt();
        }
        if toggle_dynamic_lighting {
          let _ = ctx.toggle_dynamic_lighting();
        }
        if paused {
          // If we were paused, add to a
          let start = Instant::now();
          ctx.wait_key_pressed();
          paused_time += start.elapsed();
        }
      }

      let round_time = start.elapsed() - paused_time;
      let total_time = settings.options.round_time;
      let remaining = total_time.checked_sub(round_time).unwrap_or(Duration::ZERO);

      // Sudden death management for desktop
      if !world.campaign_mode {
        if remaining <= Duration::from_secs(60) {
          world.sudden_death_warning = true;
        }
        if remaining <= Duration::from_secs(50) {
          let shrink_step = ((50 - remaining.as_secs()) / 5) as u16;
          while world.sudden_death_ring < shrink_step {
            world.advance_sudden_death_shrink();
            ctx.gamepad.rumble_all(1.0, 500);
          }
        }
      }

      // Apply all rendering updates
      ctx.with_render_context(|canvas| {
        if world.update.players_info {
          self.render_players_info(canvas, &world)?;
          if world.campaign_mode {
            self.render_lives(canvas, world.players.len() as i32, world.players[0].lives)?;
          }
          world.update.players_info = false;
        }

        // Go through each update and render it
        for update in &world.update.queue {
          match *update {
            Update::Actor(actor, digging) => {
              let cheat = if actor < world.players.len() {
                world.players[actor].glyph_cheat()
              } else {
                None
              };
              let actor = &world.actors[actor];
              self.render_actor(canvas, actor, cheat, digging)?;
            }
            Update::Map(cursor) => {
              self.reveal_map_square(canvas, cursor, &mut world.maps)?;
            }
            Update::Border(cursor) => {
              self.render_dirt_border(canvas, cursor, &world.maps.level)?;
            }
            Update::BurnedBorder(cursor) => {
              self.render_burned_border(canvas, cursor, &world.maps.level)?;
            }
            Update::Splatter(cursor, dir, splatter) => {
              self.render_splatter(canvas, cursor, dir, splatter)?;
            }
          }
        }

        // Update end of round indicator
        if !world.campaign_mode {
          let width = ((635 * round_time.as_millis()) / settings.options.round_time.as_millis()).min(635) as i32;
          if world.sudden_death_active {
            canvas.set_draw_color(Color::RGB(240, 40, 30));
          } else if world.sudden_death_warning {
            let flash = (round_time.as_millis() / 250) % 2 == 0;
            if flash {
              canvas.set_draw_color(Color::RGB(255, 40, 40));
            } else {
              canvas.set_draw_color(Color::RGB(255, 210, 30));
            }
          } else {
            canvas.set_draw_color(self.players.palette[0]);
          }
          canvas
            .fill_rect(Rect::new(636 - width, 473, width as u32, 5))
            .map_err(SdlError)?;
        }

        world.update.queue.clear();
        Ok(())
      })?;

      if !world.campaign_mode && round_time >= settings.options.round_time {
        break RoundEnd::Round;
      }

      if world.is_end_of_round() {
        if world.campaign_mode && world.alive_players() == 0 {
          break RoundEnd::Failed;
        }
        break RoundEnd::Round;
      }

      // Register any new explosions from effects queue for dynamic lighting
      for request in &world.effects.queue {
        match request.effect {
          SoundEffect::Explos1 | SoundEffect::Explos2 | SoundEffect::Explos4 | SoundEffect::Explos5 => {
            let pos = request.location.position();
            active_explosions.push((pos.x, pos.y, 10));
          }
          SoundEffect::Explos3 => {
            let pos = request.location.position();
            active_explosions.push((pos.x, pos.y, 20));
          }
          _ => {}
        }
      }

      // Collect active dynamic light sources
      let mut expl_lights = Vec::new();
      active_explosions.retain_mut(|(x, y, frames)| {
        let intensity = *frames as f32 / 10.0;
        expl_lights.push((*x, *y, intensity));
        *frames = frames.saturating_sub(1);
        *frames > 0
      });

      if ctx.config.graphics.dynamic_lighting {
        let mut lanterns = Vec::with_capacity(world.actors.len());
        for actor in &world.actors {
          if !actor.is_dead && actor.health > 0 {
            lanterns.push((actor.pos.x, actor.pos.y));
          }
        }
        let _ = ctx.apply_dynamic_lighting(&lanterns, &expl_lights);
      }

      if world.flash {
        ctx.present_flash()?;
        ctx.gamepad.rumble_all(1.0, 350);
      } else if world.shake % 2 != 0 {
        ctx.present_shake(world.shake)?;
        let intensity = (f32::from(world.shake) / 20.0).min(1.0).max(0.3);
        ctx.gamepad.rumble_all(intensity, 150);
      } else {
        ctx.present()?;
      }

      // Play sound effects and trigger localized rumble for nearby explosions
      for request in &world.effects.queue {
        self.effects.play(request.effect, request.frequency, request.location)?;
        match request.effect {
          SoundEffect::Explos1 | SoundEffect::Explos2 | SoundEffect::Explos4 | SoundEffect::Explos5 => {
            let expl_col = request.location.col;
            let expl_row = request.location.row;
            for (p_idx, player) in world.players.iter().enumerate() {
              if player.is_bot || p_idx >= world.actors.len() {
                continue;
              }
              let p_cursor = world.actors[p_idx].pos.cursor();
              let dc = (expl_col as f32 - p_cursor.col as f32).abs();
              let dr = (expl_row as f32 - p_cursor.row as f32).abs();
              let dist: f32 = (dc * dc + dr * dr).sqrt();
              if dist < 20.0 {
                let intensity: f32 = (1.0f32 - (dist / 20.0f32)).max(0.25f32);
                ctx.gamepad.rumble_player(p_idx, intensity, 180);
              }
            }
          }
          SoundEffect::Explos3 => {
            ctx.gamepad.rumble_all(1.0, 400);
          }
          SoundEffect::Aargh | SoundEffect::Karjaisu => {
            for (p_idx, player) in world.players.iter().enumerate() {
              if !player.is_bot && p_idx < world.actors.len() && world.actors[p_idx].pos.cursor() == request.location {
                ctx.gamepad.rumble_player(p_idx, 0.85, 250);
              }
            }
          }
          _ => {}
        }
      }
      world.effects.queue.clear();

      let frame_delay_ms = ((base_tick_ms / speed_multiplier).max(2.0)) as u64;
      std::thread::sleep(std::time::Duration::from_millis(frame_delay_ms));
    };

    sdl2::mixer::Music::halt();
    ctx.animate(Animation::FadeDown, 7)?;

    world.end_of_round();
    Ok(exit_reason)
  }

  fn render_game_screen(&self, canvas: &mut WindowCanvas, world: &World) -> Result<(), anyhow::Error> {
    canvas.copy(&self.players.texture, None, None).map_err(SdlError)?;

    self.render_level(canvas, &world.maps.level, world.maps.darkness)?;
    if world.maps.darkness {
      canvas.set_draw_color(Color::BLACK);
      canvas.fill_rect(Rect::new(10, 40, 620, 430)).map_err(SdlError)?;
    } else {
      // Render actors
      for (idx, actor) in world.actors.iter().enumerate() {
        let cheat = if idx < world.players.len() {
          world.players[idx].glyph_cheat()
        } else {
          None
        };
        self.render_actor(canvas, actor, cheat, Digging::Hands)?;
      }
    }

    self.render_players_info(canvas, world)?;
    if world.campaign_mode {
      self.render_lives(canvas, world.players.len() as i32, world.players[0].lives)?;
    } else {
      // Time bar
      canvas.set_draw_color(self.players.palette[6]);
      canvas.fill_rect(Rect::new(2, 473, 635, 5)).map_err(SdlError)?;
    }
    Ok(())
  }

  fn render_level(&self, canvas: &mut WindowCanvas, level: &LevelMap, darkness: bool) -> Result<(), anyhow::Error> {
    let mut render = |cursor: Cursor| {
      let glyph = Glyph::Map(level[cursor]);
      let pos = cursor.position();
      self
        .glyphs
        .render(canvas, i32::from(pos.x) - 5, i32::from(pos.y) - 5, glyph)
    };
    if darkness {
      // Only render borders
      for row in 0..MAP_ROWS {
        render(Cursor::new(row, 0))?;
        render(Cursor::new(row, MAP_COLS - 1))?;
      }
      for col in 0..MAP_COLS {
        render(Cursor::new(0, col))?;
        render(Cursor::new(MAP_ROWS - 1, col))?;
      }
    } else {
      // Render everything
      for cursor in Cursor::all() {
        render(cursor)?;
      }

      // Render dirt borders
      for cursor in Cursor::all_without_borders() {
        if DIRT_BORDER_BITMAP[level[cursor]] {
          self.render_dirt_border(canvas, cursor, level)?;
        }
      }
    }
    Ok(())
  }

  /// Render smoothed border for both stone and dirt blocks
  fn render_dirt_border(
    &self,
    canvas: &mut WindowCanvas,
    cursor: Cursor,
    level: &LevelMap,
  ) -> Result<(), anyhow::Error> {
    let pos = cursor.position();
    let pos_x = i32::from(pos.x);
    let pos_y = i32::from(pos.y);

    // Dirt
    for dir in Direction::all() {
      let value = level[cursor.to(dir)];
      let is_corner = match dir {
        Direction::Right if value == MapValue::StoneTopLeft || value == MapValue::StoneBottomLeft => true,
        Direction::Left if value == MapValue::StoneTopRight || value == MapValue::StoneBottomRight => true,
        Direction::Down if value == MapValue::StoneTopLeft || value == MapValue::StoneTopRight => true,
        Direction::Up if value == MapValue::StoneBottomRight || value == MapValue::StoneBottomLeft => true,
        _ => false,
      };
      if (value >= MapValue::Sand1 && value <= MapValue::HeavyGravel) || is_corner {
        let (dx, dy) = border_offset(dir);
        self.glyphs.render(
          canvas,
          pos_x + dx,
          pos_y + dy,
          Glyph::SandBorder(dir.reverse(), Border::Normal),
        )?;
      }
    }

    // Stone
    for dir in Direction::all() {
      let value = level[cursor.to(dir)];
      if value.is_stone() {
        let (dx, dy) = border_offset(dir);
        self.glyphs.render(
          canvas,
          pos_x + dx,
          pos_y + dy,
          Glyph::StoneBorder(dir.reverse(), Border::Normal),
        )?;
      }
    }
    Ok(())
  }

  /// Render burned border for both stone and dirt blocks
  fn render_burned_border(
    &self,
    canvas: &mut WindowCanvas,
    cursor: Cursor,
    level: &LevelMap,
  ) -> Result<(), anyhow::Error> {
    let pos = cursor.position();
    let pos_x = i32::from(pos.x);
    let pos_y = i32::from(pos.y);

    let value = level[cursor];
    if value == MapValue::Explosion || value == MapValue::MonsterDying {
      for dir in Direction::all() {
        let value = level[cursor.to(dir)];
        let glyph = if value.is_sand() || value == MapValue::LightGravel || value == MapValue::HeavyGravel {
          Glyph::SandBorder(dir.reverse(), Border::Burned)
        } else if value.is_stone() || value.is_stone_corner() {
          Glyph::StoneBorder(dir.reverse(), Border::Burned)
        } else {
          continue;
        };
        let (dx, dy) = border_offset(dir);
        self.glyphs.render(canvas, pos_x + dx, pos_y + dy, glyph)?;
      }
    } else if value == MapValue::HeavyGravel {
      // FIXME: not sure when this one is triggered?
      for dir in Direction::all() {
        let value = level[cursor.to(dir)];
        if value.is_passable() || value == MapValue::Explosion || value == MapValue::MonsterDying {
          let (dx, dy) = border_offset(dir);
          self.glyphs.render(
            canvas,
            pos_x + dx,
            pos_y + dy,
            Glyph::SandBorder(dir.reverse(), Border::Burned),
          )?;
        }
      }
    }
    Ok(())
  }

  fn render_splatter(
    &self,
    canvas: &mut WindowCanvas,
    cursor: Cursor,
    dir: Direction,
    splatter: SplatterKind,
  ) -> Result<(), anyhow::Error> {
    let mut rng = rand::thread_rng();
    let color = match splatter {
      SplatterKind::Blood => 3,
      SplatterKind::Slime => 4,
    };
    canvas.set_draw_color(self.players.palette[color]);
    let pos = cursor.position();
    loop {
      let (delta_x, delta_y) = match dir {
        Direction::Left => (-5 - rng.gen_range(0..3), rng.gen_range(-5..5)),
        Direction::Right => (5 + rng.gen_range(0..3), rng.gen_range(-5..5)),
        Direction::Up => (rng.gen_range(-5..5), -5 - rng.gen_range(0..3)),
        Direction::Down => (rng.gen_range(-5..5), 5 + rng.gen_range(0..3)),
      };
      canvas
        .draw_point((i32::from(pos.x) + delta_x, i32::from(pos.y) + delta_y))
        .map_err(SdlError)?;
      if rng.gen_range(0..10) == 0 {
        break;
      }
    }
    Ok(())
  }

  fn render_players_info(&self, canvas: &mut WindowCanvas, world: &World) -> Result<(), anyhow::Error> {
    // Erase extra players
    let players_len = world.players.len() as u16;
    if players_len < 4 {
      let rect = Rect::new(i32::from(players_len) * 160, 0, u32::from(4 - players_len) * 160, 30);
      canvas.set_draw_color(Color::BLACK);
      canvas.fill_rect(rect).map_err(SdlError)?;
    }

    // Current weapon selection
    const PLAYER_X: [i32; 4] = [12, 174, 337, 500];
    let palette = &self.players.palette;
    for (idx, player) in world.players.iter().enumerate() {
      let pos_x = PLAYER_X[idx];
      self
        .glyphs
        .render(canvas, pos_x, 0, Glyph::Selection(player.selection))?;
      self.font.render(
        canvas,
        pos_x,
        0,
        palette[1],
        &player.inventory[player.selection].to_string(),
      )?;

      canvas.set_draw_color(Color::BLACK);
      canvas.fill_rect(Rect::new(pos_x + 50, 11, 40, 8)).map_err(SdlError)?;
      self.font.render(
        canvas,
        pos_x + 50,
        11,
        palette[3],
        &world.actors[idx].drilling.to_string(),
      )?;
      self
        .font
        .render(canvas, pos_x + 36, 1, palette[1], &player.stats.name)?;

      canvas.set_draw_color(Color::BLACK);
      canvas.fill_rect(Rect::new(pos_x + 50, 21, 40, 8)).map_err(SdlError)?;

      let cash_idx = if world.campaign_mode { 0 } else { idx };
      let total_cash = world.players[cash_idx].cash + world.actors[cash_idx].accumulated_cash;
      self
        .font
        .render(canvas, pos_x + 50, 21, palette[5], &total_cash.to_string())?;
    }

    // Players health
    const HEALTH_COLOR: [usize; 4] = [2, 3, 4, 6];
    const HEALTH_BAR_LEFT: [i32; 4] = [142, 304, 467, 630];
    for player in 0..world.players.len() {
      let actor = &world.actors[player];
      let health_bars = if actor.health == 0 {
        0
      } else {
        (u32::from(actor.health) * 50 + 1) / (2 * u32::from(actor.max_health)) + 1
      };
      let left = HEALTH_BAR_LEFT[player];
      canvas.set_draw_color(Color::BLACK);
      if health_bars < 25 {
        canvas
          .fill_rect(Rect::new(left, 2, 8, 26 - health_bars))
          .map_err(SdlError)?;
      }
      if health_bars > 0 {
        canvas.set_draw_color(palette[HEALTH_COLOR[player]]);
        canvas
          .fill_rect(Rect::new(left, 28 - (health_bars as i32), 8, health_bars))
          .map_err(SdlError)?;
      }
    }
    Ok(())
  }

  fn render_lives(&self, canvas: &mut WindowCanvas, players: i32, lives: u16) -> Result<(), anyhow::Error> {
    canvas.set_draw_color(Color::BLACK);
    canvas
      .fill_rect(Rect::new(160 * players, 2, 480, 28))
      .map_err(SdlError)?;
    for idx in 0..lives.max(3) {
      let glyph = if idx < lives { Glyph::Life } else { Glyph::LifeLost };
      self
        .glyphs
        .render(canvas, i32::from(idx * 16) + 160 * players, 2, glyph)?;
    }
    Ok(())
  }

  fn render_actor(
    &self,
    canvas: &mut WindowCanvas,
    actor: &ActorComponent,
    cheat: Option<GlyphCheat>,
    digging: Digging,
  ) -> Result<(), anyhow::Error> {
    let phase = match actor.animation / 5 {
      _ if !actor.moving => AnimationPhase::Phase1,
      0 => AnimationPhase::Phase1,
      1 => AnimationPhase::Phase2,
      2 => AnimationPhase::Phase3,
      3 => AnimationPhase::Phase4,
      4 => AnimationPhase::Phase3,
      _ => AnimationPhase::Phase2,
    };

    let pos_x = i32::from(actor.pos.x) - 5;
    let pos_y = i32::from(actor.pos.y) - 5;
    // Check for glyph-related cheat codes

    let kind = match cheat {
      None => actor.kind,
      Some(GlyphCheat::Slime) => ActorKind::Slime,
      Some(GlyphCheat::Invisible) => return Ok(()),
    };
    let glyph = Glyph::Monster(kind, actor.facing, digging, phase);
    self.glyphs.render(canvas, pos_x, pos_y, glyph)?;
    Ok(())
  }

  fn reveal_map_square(&self, canvas: &mut WindowCanvas, cursor: Cursor, maps: &mut Maps) -> Result<(), anyhow::Error> {
    // FIXME: temporary. Need to figure out what to do with time bar
    if cursor.row == MAP_ROWS - 1 {
      return Ok(());
    }

    let glyph = Glyph::Map(maps.level[cursor]);
    let pos = cursor.position();
    self
      .glyphs
      .render(canvas, i32::from(pos.x) - 5, i32::from(pos.y) - 5, glyph)?;
    // FIXME: move to world?
    maps.fog[cursor].reveal();
    Ok(())
  }
}

fn border_offset(dir: Direction) -> (i32, i32) {
  match dir {
    Direction::Left => (-9, -5),
    Direction::Right => (5, -5),
    Direction::Up => (-5, -8),
    Direction::Down => (-5, 5),
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerWin {
  Lose,
  Draw,
  Win,
}

fn compute_score(players: &[PlayerComponent], player: usize, win: WinCondition) -> PlayerWin {
  let scorefn = |player: &PlayerComponent| match win {
    WinCondition::ByWins => player.rounds_win,
    WinCondition::ByMoney | WinCondition::GoldRush | WinCondition::Survival => player.cash,
  };
  let score = scorefn(&players[player]);
  let bested_by = players.iter().filter(|player| scorefn(player) > score).count();
  if bested_by == 0 {
    PlayerWin::Win
  } else if bested_by == players.len() - 1 {
    PlayerWin::Lose
  } else {
    PlayerWin::Draw
  }
}

fn update_player_stats(
  game_dir: &Path,
  players: &mut [PlayerComponent],
  player_to_roster: &[u8],
  win: WinCondition,
) -> Result<(), anyhow::Error> {
  let mut roster = PlayersRoster::load(game_dir)?;
  for idx in 0..players.len() {
    let is_win = compute_score(players, idx, win) == PlayerWin::Win;
    let stats = &mut players[idx].stats;
    let tournament = stats.tournaments as usize;
    let history_len = stats.history.len();
    stats.history[tournament % history_len] = 123;
    stats.tournaments += 1;
    if is_win {
      stats.tournaments_wins += 1;
    }

    if let Some(roster_stats) = roster.players[usize::from(player_to_roster[idx])].as_mut() {
      roster_stats.update_stats_tournament(stats);
    }
  }
  roster.save(game_dir)?;
  Ok(())
}
