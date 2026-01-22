#[macro_export]
macro_rules! strategy_new {
    ($type:ident, $strategy_id:expr) => {
        impl $type {
            pub fn new() -> Self {
                let config = StrategyConfig {
                    min_profit_bps: StrategyLimits::get_min_profit_bps($strategy_id),
                    max_exposure: StrategyLimits::get_max_exposure($strategy_id),
                    custom_params: StrategyUtils::create_base_custom_params(),
                    ..Default::default()
                };
                Self { config }
            }
        }
    };
}

#[macro_export]
macro_rules! strategy_default {
    ($type:ident) => {
        impl Default for $type {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}
