use super::{rawir, Message};

impl Message {
    /// Create an empty packet
    pub fn new() -> Self {
        Message::default()
    }

    /// Concatenate to packets
    pub fn extend(&mut self, other: &Message) {
        if self.carrier.is_none() {
            self.carrier = other.carrier;
        }

        if self.duty_cycle.is_none() {
            self.duty_cycle = other.duty_cycle;
        }

        self.raw.extend_from_slice(&other.raw);
    }

    /// Do we have a trailing gap
    pub fn has_trailing_gap(&self) -> bool {
        let len = self.raw.len();

        len > 0 && (len % 2) == 0
    }

    /// Remove any trailing gap
    pub fn remove_trailing_gap(&mut self) {
        if self.has_trailing_gap() {
            self.raw.pop();
        }
    }

    /// Print the flash and gap information as an raw ir string
    pub fn print_rawir(&self) -> String {
        rawir::print_to_string(&self.raw)
    }
}
