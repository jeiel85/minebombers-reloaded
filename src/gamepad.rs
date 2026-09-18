use crate::keys::Key;
use sdl2::controller::{Axis, Button, GameController};
use sdl2::event::Event;
use sdl2::GameControllerSubsystem;
use std::collections::HashMap;

const ANALOG_DEADZONE: i16 = 12000;

/// Manages connected SDL2 game controllers and maps them to player actions
pub struct GamepadManager {
  subsystem: GameControllerSubsystem,
  /// Maps controller instance ID to player index (0..3) and opened GameController
  controllers: HashMap<u32, (usize, GameController)>,
  /// Slot tracker: whether player slot 0..3 has a gamepad assigned
  assigned_slots: [Option<u32>; 4],
}

impl GamepadManager {
  pub fn new(subsystem: GameControllerSubsystem) -> Self {
    let mut manager = Self {
      subsystem,
      controllers: HashMap::new(),
      assigned_slots: [None; 4],
    };
    manager.scan_controllers();
    manager
  }

  /// Scan and open all currently plugged controllers
  pub fn scan_controllers(&mut self) {
    let num_joysticks = self.subsystem.num_joysticks().unwrap_or(0);
    for idx in 0..num_joysticks {
      if self.subsystem.is_game_controller(idx) {
        self.add_controller(idx);
      }
    }
  }

  /// Add a newly connected controller
  pub fn add_controller(&mut self, joystick_index: u32) {
    // Find an unassigned player slot (0..3)
    let slot = match self.assigned_slots.iter().position(|s| s.is_none()) {
      Some(s) => s,
      None => return, // Max 4 players
    };

    if let Ok(controller) = self.subsystem.open(joystick_index) {
      let instance_id = controller.instance_id();
      println!(
        "[GAMEPAD] Connected: '{}' assigned to Player {}",
        controller.name(),
        slot + 1
      );
      self.assigned_slots[slot] = Some(instance_id);
      self.controllers.insert(instance_id, (slot, controller));
    }
  }

  /// Remove a disconnected controller
  pub fn remove_controller(&mut self, instance_id: u32) {
    if let Some((slot, controller)) = self.controllers.remove(&instance_id) {
      println!(
        "[GAMEPAD] Disconnected: '{}' from Player {}",
        controller.name(),
        slot + 1
      );
      self.assigned_slots[slot] = None;
    }
  }

  /// Check if any gamepad is connected
  pub fn has_controllers(&self) -> bool {
    !self.controllers.is_empty()
  }

  /// Check which key actions are currently pressed on the controller for a given player
  pub fn get_player_actions(&self, player_idx: usize) -> Vec<Key> {
    let mut actions = Vec::new();
    let instance_id = match self.assigned_slots[player_idx] {
      Some(id) => id,
      None => return actions,
    };

    if let Some((_, controller)) = self.controllers.get(&instance_id) {
      // D-Pad and Left Analog Stick navigation
      let axis_x = controller.axis(Axis::LeftX);
      let axis_y = controller.axis(Axis::LeftY);

      if controller.button(Button::DPadUp) || axis_y < -ANALOG_DEADZONE {
        actions.push(Key::Up);
      }
      if controller.button(Button::DPadDown) || axis_y > ANALOG_DEADZONE {
        actions.push(Key::Down);
      }
      if controller.button(Button::DPadLeft) || axis_x < -ANALOG_DEADZONE {
        actions.push(Key::Left);
      }
      if controller.button(Button::DPadRight) || axis_x > ANALOG_DEADZONE {
        actions.push(Key::Right);
      }

      // Action buttons
      // A (South): Bomb
      if controller.button(Button::A) {
        actions.push(Key::Bomb);
      }
      // X (West): Choose/cycle weapon
      if controller.button(Button::X) {
        actions.push(Key::Choose);
      }
      // Y (North) or RB: Remote detonator
      if controller.button(Button::Y) || controller.button(Button::RightShoulder) {
        actions.push(Key::Remote);
      }
      // B (East) or LB: Stop / Hold still
      if controller.button(Button::B) || controller.button(Button::LeftShoulder) {
        actions.push(Key::Stop);
      }
    }
    actions
  }

  /// Trigger haptic rumble on a player's controller
  /// `intensity`: 0.0 (none) to 1.0 (maximum)
  /// `duration_ms`: duration in milliseconds
  pub fn rumble_player(&mut self, player_idx: usize, intensity: f32, duration_ms: u32) {
    let instance_id = match self.assigned_slots[player_idx] {
      Some(id) => id,
      None => return,
    };

    if let Some((_, controller)) = self.controllers.get_mut(&instance_id) {
      let clamped = intensity.max(0.0).min(1.0);
      let strength = (clamped * 65535.0) as u16;
      let _ = controller.set_rumble(strength, strength, duration_ms);
    }
  }

  /// Rumble all active controllers (e.g. for huge explosions like Atomic Bomb)
  pub fn rumble_all(&mut self, intensity: f32, duration_ms: u32) {
    let clamped = intensity.max(0.0).min(1.0);
    let strength = (clamped * 65535.0) as u16;
    for (_, controller) in self.controllers.values_mut() {
      let _ = controller.set_rumble(strength, strength, duration_ms);
    }
  }

  /// Handle SDL controller events (connecting / disconnecting)
  pub fn handle_event(&mut self, event: &Event) {
    match event {
      Event::ControllerDeviceAdded { which, .. } => {
        self.add_controller(*which);
      }
      Event::ControllerDeviceRemoved { which, .. } => {
        self.remove_controller(*which);
      }
      _ => {}
    }
  }
}
