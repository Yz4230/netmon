pub trait NumericFormatter {
    fn humanize_size(self) -> String;
    fn humanize_bps(self) -> String;
}

impl<T: Into<f64>> NumericFormatter for T {
    fn humanize_size(self) -> String {
        let size = self.into();
        const UNITS: [&str; 9] = ["", "K", "M", "G", "T", "P", "E", "Z", "Y"];
        let unit = (0..=UNITS.len())
            .find(|i| size < (1024f64).powi(*i as i32))
            .unwrap_or(UNITS.len())
            .saturating_sub(1);

        format!("{:.1}{}", size / (1024f64).powi(unit as i32), UNITS[unit])
    }

    fn humanize_bps(self) -> String {
        format!("{}bps", self.humanize_size())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_humanize_size() {
        assert_eq!(0.humanize_size(), "0.0");
        assert_eq!(1.humanize_size(), "1.0");
        assert_eq!(1024.humanize_size(), "1.0K");
        assert_eq!(1024f64.powi(2).humanize_size(), "1.0M");
        assert_eq!(1024f64.powi(3).humanize_size(), "1.0G");
        assert_eq!(1024f64.powi(4).humanize_size(), "1.0T");
        assert_eq!(1024f64.powi(5).humanize_size(), "1.0P");
        assert_eq!(1024f64.powi(6).humanize_size(), "1.0E");
        assert_eq!(1024f64.powi(7).humanize_size(), "1.0Z");
        assert_eq!(1024f64.powi(8).humanize_size(), "1.0Y");
    }
}
