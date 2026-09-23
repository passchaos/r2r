fn main() -> Result<(), Box<dyn std::error::Error>> {
    {
        let clock = r2r::Clock::create(r2r::ClockType::RosTime)?;
        let now = clock.get_now()?;
        let time = r2r::Clock::try_to_builtin_time(&now)?;
        println!("rostime: {:?}", time);
    }
    {
        let clock = r2r::Clock::create(r2r::ClockType::SystemTime)?;
        let now = clock.get_now()?;
        let time = r2r::Clock::try_to_builtin_time(&now)?;
        println!("systemtime: {:?}", time);
    }
    {
        let clock = r2r::Clock::create(r2r::ClockType::SteadyTime)?;
        let now = clock.get_now()?;
        let time = r2r::Clock::try_to_builtin_time(&now)?;
        println!("steadytime: {:?}", time);
    }
    Ok(())
}
