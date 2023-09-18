use caminos_lib::*;
use config_parser::ConfigurationValue;

pub struct VirtualChannelPoliciesBuilder{

    pub policies: Vec<ConfigurationValue>,
}

pub fn create_vcp( arg: VirtualChannelPoliciesBuilder) -> ConfigurationValue
{
    ConfigurationValue::Array(arg.policies)
}

pub struct InputOutputRouterBuilder {
    pub virtual_channels: usize,
    pub vcp: ConfigurationValue,
    pub crossbar_delay: usize,
    pub crossbar_frequency_divisor: usize,
    pub allocator: ConfigurationValue,
    pub buffer_size: usize,
    pub bubble: ConfigurationValue,
    pub flit_size: usize,
    pub allow_request_busy_port: ConfigurationValue,
    pub intransit_priority: ConfigurationValue,
    pub output_buffer_size: usize,
    pub neglect_busy_outport: ConfigurationValue,
}


pub fn create_input_output_router(arg: InputOutputRouterBuilder)-> ConfigurationValue
{
        ConfigurationValue::Object("InputOutput".to_string(), vec![
            ("virtual_channels".to_string(),ConfigurationValue::Number( arg.virtual_channels as f64 )),
            ("virtual_channel_policies".to_string(), arg.vcp),
            ("allocator".to_string(), arg.allocator),
            ("crossbar_delay".to_string(), ConfigurationValue::Number(arg.crossbar_delay as f64) ),
            ("crossbar_frequency_divisor".to_string(), ConfigurationValue::Number(arg.crossbar_frequency_divisor as f64)),
            ("buffer_size".to_string(), ConfigurationValue::Number(arg.buffer_size as f64)),
            ("bubble".to_string(), arg.bubble),
            ("flit_size".to_string(), ConfigurationValue::Number(arg.flit_size as f64) ),
            ("allow_request_busy_port".to_string(), arg.allow_request_busy_port),
            ("intransit_priority".to_string(), arg.intransit_priority),
            ("output_buffer_size".to_string(), ConfigurationValue::Number(arg.output_buffer_size as f64)),
            ("neglect_busy_output".to_string(), arg.neglect_busy_outport)
    ])
}

pub struct BasicRouterBuilder {
    pub virtual_channels: usize,
    pub vcp: ConfigurationValue,
    pub buffer_size: usize,
    pub bubble: ConfigurationValue,
    pub flit_size: usize,
    pub allow_request_busy_port: ConfigurationValue,
    pub intransit_priority: ConfigurationValue,
    pub output_buffer_size: usize,
    pub neglect_busy_outport: ConfigurationValue,
    pub output_prioritize_lowest_label: ConfigurationValue,
}

pub fn create_basic_router( arg: BasicRouterBuilder)-> ConfigurationValue
{
    ConfigurationValue::Object("Basic".to_string(), vec![
        ("virtual_channels".to_string(),ConfigurationValue::Number(arg.virtual_channels as f64)),
        ("virtual_channel_policies".to_string(), arg.vcp),
        ("buffer_size".to_string(), ConfigurationValue::Number(arg.buffer_size as f64)),
        ("bubble".to_string(), arg.bubble),
        ("flit_size".to_string(), ConfigurationValue::Number(arg.flit_size as f64) ),
        ("allow_request_busy_port".to_string(), arg.allow_request_busy_port),
        ("intransit_priority".to_string(), arg.intransit_priority),
        ("output_buffer_size".to_string(), ConfigurationValue::Number(arg.output_buffer_size as f64)),
        ("neglect_busy_output".to_string(), arg.neglect_busy_outport),
        ("output_prioritize_lowest_label".to_string(), arg.output_prioritize_lowest_label)
    ])

}

pub struct HammingBuilder
{
    pub sides: Vec<ConfigurationValue>,
    pub servers_per_router: usize,
}

pub fn create_hamming_topology(arg: HammingBuilder) -> ConfigurationValue //Box<dyn Topology>
{
    ConfigurationValue::Object("Hamming".to_string(),
                                        vec![("sides".to_string(),ConfigurationValue::Array(arg.sides)),
                                             ("servers_per_router".to_string(),ConfigurationValue::Number(arg.servers_per_router as f64))])
}

pub struct ShiftPatternBuilder
{
    pub sides: Vec<ConfigurationValue>,
    pub shift: Vec<ConfigurationValue>,
}

pub fn create_shift_pattern(arg: ShiftPatternBuilder) -> ConfigurationValue
{
    ConfigurationValue::Object("CartesianTransform".to_string(), vec![
        ("sides".to_string(),ConfigurationValue::Array(arg.sides)),
        ("shift".to_string(),ConfigurationValue::Array(arg.shift))])
}


pub struct BurstTrafficBuilder
{
    pub pattern: ConfigurationValue,
    pub servers: usize,
    pub messages_per_server: usize,
    pub message_size: usize,
}

pub fn create_burst_traffic(arg: BurstTrafficBuilder) -> ConfigurationValue
{
    ConfigurationValue::Object("Burst".to_string(), vec![("pattern".to_string(), arg.pattern ),
                                                         ("servers".to_string(), ConfigurationValue::Number(arg.servers as f64)),
                                                         ("messages_per_server".to_string(), ConfigurationValue::Number(arg.messages_per_server as f64)),
                                                         ("message_size".to_string(), ConfigurationValue::Number(arg.message_size as f64))])
}

pub fn create_shortest_routing() -> ConfigurationValue
{
    ConfigurationValue::Object("Shortest".to_string(), vec![])
}


pub fn create_link_classes() -> ConfigurationValue
{
   ConfigurationValue::Array(vec![
       ConfigurationValue::Object("LinkClass".to_string(), vec![("delay".to_string(), ConfigurationValue::Number(1.0))]),
       ConfigurationValue::Object("LinkClass".to_string(), vec![("delay".to_string(), ConfigurationValue::Number(1.0))]),
       ConfigurationValue::Object("LinkClass".to_string(), vec![("delay".to_string(), ConfigurationValue::Number(1.0))]),
       ConfigurationValue::Object("LinkClass".to_string(), vec![("delay".to_string(), ConfigurationValue::Number(1.0))]),
       ConfigurationValue::Object("LinkClass".to_string(), vec![("delay".to_string(), ConfigurationValue::Number(1.0))]),
   ])
}

pub struct SimulationBuilder
{
    pub random_seed: usize,
    pub warmup: usize,
    pub measured: usize,
    pub topology: ConfigurationValue,
    pub traffic: ConfigurationValue,
    pub router: ConfigurationValue,
    pub maximum_packet_size: usize,
    pub general_frequency_divisor: usize,
    pub routing: ConfigurationValue,
    pub link_classes: ConfigurationValue

}
pub fn create_simulation(arg: SimulationBuilder) -> ConfigurationValue
{

    ConfigurationValue::Object("Configuration".to_string(), vec![
        ("random_seed".to_string(), ConfigurationValue::Number(arg.random_seed as f64)),
        ("warmup".to_string(), ConfigurationValue::Number( arg.warmup as f64)),
        ("measured".to_string(), ConfigurationValue::Number(arg.measured as f64)),
        ("topology".to_string(), arg.topology),
        ("traffic".to_string(), arg.traffic),
        ("router".to_string(), arg.router),
        ("maximum_packet_size".to_string(), ConfigurationValue::Number(arg.maximum_packet_size as f64)),
        ("general_frequency_divisor".to_string(), ConfigurationValue::Number(arg.general_frequency_divisor as f64)),
        ("routing".to_string(), arg.routing),
        ("link_classes".to_string(), arg.link_classes),
    ])

}
