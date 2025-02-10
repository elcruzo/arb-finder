use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use std::collections::VecDeque;

pub fn calculate_percentage_change(old_value: Decimal, new_value: Decimal) -> Decimal {
    if old_value.is_zero() {
        return Decimal::ZERO;
    }
    ((new_value - old_value) / old_value) * Decimal::from(100)
}

pub fn calculate_basis_points(value: Decimal, base: Decimal) -> i32 {
    if base.is_zero() {
        return 0;
    }
    let percentage = (value / base) * Decimal::from(10000);
    percentage.to_i32().unwrap_or(0)
}

pub fn basis_points_to_decimal(bps: i32) -> Decimal {
    Decimal::from(bps) / Decimal::from(10000)
}

pub fn round_to_tick_size(price: Decimal, tick_size: Decimal) -> Decimal {
    if tick_size.is_zero() {
        return price;
    }
    (price / tick_size).round() * tick_size
}

pub fn round_to_lot_size(quantity: Decimal, lot_size: Decimal) -> Decimal {
    if lot_size.is_zero() {
        return quantity;
    }
    (quantity / lot_size).floor() * lot_size
}

pub fn calculate_notional_value(price: Decimal, quantity: Decimal) -> Decimal {
    price * quantity
}

pub fn calculate_weighted_average_price(prices: &[(Decimal, Decimal)]) -> Decimal {
    if prices.is_empty() {
        return Decimal::ZERO;
    }

    let mut total_value = Decimal::ZERO;
    let mut total_quantity = Decimal::ZERO;

    for (price, quantity) in prices {
        total_value += price * quantity;
        total_quantity += quantity;
    }

    if total_quantity.is_zero() {
        Decimal::ZERO
    } else {
        total_value / total_quantity
    }
}

pub fn calculate_sharpe_ratio(returns: &[f64], risk_free_rate: f64) -> f64 {
    if returns.is_empty() {
        return 0.0;
    }

    let excess_returns: Vec<f64> = returns.iter().map(|r| r - risk_free_rate).collect();
    let mean_excess_return = excess_returns.iter().sum::<f64>() / excess_returns.len() as f64;
    let variance = excess_returns.iter()
        .map(|r| (r - mean_excess_return).powi(2))
        .sum::<f64>() / excess_returns.len() as f64;
    
    let std_dev = variance.sqrt();
    
    if std_dev == 0.0 {
        0.0
    } else {
        mean_excess_return / std_dev
    }
}

pub fn calculate_sortino_ratio(returns: &[f64], target_return: f64) -> f64 {
    if returns.is_empty() {
        return 0.0;
    }

    let excess_returns: Vec<f64> = returns.iter().map(|r| r - target_return).collect();
    let mean_excess_return = excess_returns.iter().sum::<f64>() / excess_returns.len() as f64;
    
    let downside_variance = excess_returns.iter()
        .filter(|&&r| r < 0.0)
        .map(|r| r.powi(2))
        .sum::<f64>() / excess_returns.len() as f64;
    
    let downside_deviation = downside_variance.sqrt();
    
    if downside_deviation == 0.0 {
        0.0
    } else {
        mean_excess_return / downside_deviation
    }
}

pub fn calculate_maximum_drawdown(values: &[Decimal]) -> Decimal {
    if values.is_empty() {
        return Decimal::ZERO;
    }

    let mut max_drawdown = Decimal::ZERO;
    let mut peak = values[0];

    for &value in values.iter().skip(1) {
        if value > peak {
            peak = value;
        } else {
            let drawdown = (peak - value) / peak;
            if drawdown > max_drawdown {
                max_drawdown = drawdown;
            }
        }
    }

    max_drawdown
}

pub fn calculate_var(returns: &[f64], confidence_level: f64) -> f64 {
    if returns.is_empty() {
        return 0.0;
    }

    let mut sorted_returns = returns.to_vec();
    sorted_returns.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    let index = ((1.0 - confidence_level) * sorted_returns.len() as f64) as usize;
    sorted_returns.get(index).copied().unwrap_or(0.0)
}

pub fn exponential_moving_average(values: &[f64], alpha: f64) -> Vec<f64> {
    if values.is_empty() {
        return Vec::new();
    }

    let mut ema = Vec::with_capacity(values.len());
    ema.push(values[0]);

    for &value in values.iter().skip(1) {
        let new_ema = alpha * value + (1.0 - alpha) * ema.last().unwrap();
        ema.push(new_ema);
    }

    ema
}

pub fn simple_moving_average(values: &[f64], window: usize) -> Vec<f64> {
    if values.len() < window {
        return Vec::new();
    }

    let mut sma = Vec::new();
    let mut sum = values.iter().take(window).sum::<f64>();
    sma.push(sum / window as f64);

    for i in window..values.len() {
        sum = sum - values[i - window] + values[i];
        sma.push(sum / window as f64);
    }

    sma
}

pub fn rolling_standard_deviation(values: &[f64], window: usize) -> Vec<f64> {
    if values.len() < window {
        return Vec::new();
    }

    let mut std_devs = Vec::new();
    
    for i in 0..=(values.len() - window) {
        let window_values = &values[i..i + window];
        let mean = window_values.iter().sum::<f64>() / window as f64;
        let variance = window_values.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / window as f64;
        std_devs.push(variance.sqrt());
    }

    std_devs
}

pub fn bollinger_bands(values: &[f64], window: usize, num_std: f64) -> Vec<(f64, f64, f64)> {
    let sma = simple_moving_average(values, window);
    let std_devs = rolling_standard_deviation(values, window);
    
    if sma.len() != std_devs.len() {
        return Vec::new();
    }

    sma.iter()
        .zip(std_devs.iter())
        .map(|(ma, std)| {
            let upper = ma + num_std * std;
            let lower = ma - num_std * std;
            (lower, *ma, upper)
        })
        .collect()
}

pub fn correlation_coefficient(x: &[f64], y: &[f64]) -> f64 {
    if x.len() != y.len() || x.is_empty() {
        return 0.0;
    }

    let n = x.len() as f64;
    let sum_x = x.iter().sum::<f64>();
    let sum_y = y.iter().sum::<f64>();
    let sum_xx = x.iter().map(|v| v * v).sum::<f64>();
    let sum_yy = y.iter().map(|v| v * v).sum::<f64>();
    let sum_xy = x.iter().zip(y.iter()).map(|(a, b)| a * b).sum::<f64>();

    let numerator = n * sum_xy - sum_x * sum_y;
    let denominator = ((n * sum_xx - sum_x * sum_x) * (n * sum_yy - sum_y * sum_y)).sqrt();

    if denominator == 0.0 {
        0.0
    } else {
        numerator / denominator
    }
}

pub fn linear_regression(x: &[f64], y: &[f64]) -> Option<(f64, f64)> {
    if x.len() != y.len() || x.len() < 2 {
        return None;
    }

    let n = x.len() as f64;
    let sum_x = x.iter().sum::<f64>();
    let sum_y = y.iter().sum::<f64>();
    let sum_xx = x.iter().map(|v| v * v).sum::<f64>();
    let sum_xy = x.iter().zip(y.iter()).map(|(a, b)| a * b).sum::<f64>();

    let denominator = n * sum_xx - sum_x * sum_x;
    if denominator == 0.0 {
        return None;
    }

    let slope = (n * sum_xy - sum_x * sum_y) / denominator;
    let intercept = (sum_y - slope * sum_x) / n;

    Some((slope, intercept))
}

#[derive(Debug)]
pub struct RollingStatistics {
    values: VecDeque<f64>,
    window_size: usize,
    sum: f64,
    sum_squares: f64,
}

impl RollingStatistics {
    pub fn new(window_size: usize) -> Self {
        Self {
            values: VecDeque::with_capacity(window_size),
            window_size,
            sum: 0.0,
            sum_squares: 0.0,
        }
    }

    pub fn add(&mut self, value: f64) {
        if self.values.len() == self.window_size {
            let old_value = self.values.pop_front().unwrap();
            self.sum -= old_value;
            self.sum_squares -= old_value * old_value;
        }

        self.values.push_back(value);
        self.sum += value;
        self.sum_squares += value * value;
    }

    pub fn mean(&self) -> f64 {
        if self.values.is_empty() {
            0.0
        } else {
            self.sum / self.values.len() as f64
        }
    }

    pub fn variance(&self) -> f64 {
        if self.values.len() < 2 {
            0.0
        } else {
            let n = self.values.len() as f64;
            let mean = self.mean();
            (self.sum_squares - n * mean * mean) / (n - 1.0)
        }
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn count(&self) -> usize {
        self.values.len()
    }

    pub fn is_full(&self) -> bool {
        self.values.len() == self.window_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_percentage_change() {
        assert_eq!(calculate_percentage_change(Decimal::from(100), Decimal::from(110)), Decimal::from(10));
        assert_eq!(calculate_percentage_change(Decimal::from(100), Decimal::from(90)), Decimal::from(-10));
        assert_eq!(calculate_percentage_change(Decimal::ZERO, Decimal::from(100)), Decimal::ZERO);
    }

    #[test]
    fn test_calculate_basis_points() {
        assert_eq!(calculate_basis_points(Decimal::from(1), Decimal::from(100)), 100);
        assert_eq!(calculate_basis_points(Decimal::from(5), Decimal::from(100)), 500);
    }

    #[test]
    fn test_round_to_tick_size() {
        let price = "123.456".parse::<Decimal>().unwrap();
        let tick_size = "0.01".parse::<Decimal>().unwrap();
        let rounded = round_to_tick_size(price, tick_size);
        assert_eq!(rounded, "123.46".parse::<Decimal>().unwrap());
    }

    #[test]
    fn test_weighted_average_price() {
        let prices = vec![
            (Decimal::from(100), Decimal::from(10)),
            (Decimal::from(110), Decimal::from(20)),
            (Decimal::from(120), Decimal::from(30)),
        ];
        let wap = calculate_weighted_average_price(&prices);
        assert!(wap > Decimal::from(110) && wap < Decimal::from(115));
    }

    #[test]
    fn test_sharpe_ratio() {
        let returns = vec![0.1, 0.05, -0.02, 0.08, 0.12];
        let sharpe = calculate_sharpe_ratio(&returns, 0.02);
        assert!(sharpe > 0.0);
    }

    #[test]
    fn test_rolling_statistics() {
        let mut stats = RollingStatistics::new(3);
        
        stats.add(1.0);
        stats.add(2.0);
        stats.add(3.0);
        
        assert_eq!(stats.mean(), 2.0);
        assert!(stats.is_full());
        
        stats.add(4.0);
        assert_eq!(stats.mean(), 3.0);
        assert_eq!(stats.count(), 3);
    }
}