use std::cell::RefCell;
use std::rc::Rc;
use std::default::Default;
use caminos_lib::*;
use caminos_lib::policies::VirtualChannelPolicy;
use caminos_lib::router::RouterBuilderArgument;
use caminos_lib::topology::{new_topology, Topology, TopologyBuilderArgument};
use config_parser::ConfigurationValue;
use rand::rngs::StdRng;
use caminos_lib::router::input_output::InputOutput;


pub fn create_vcp() -> ConfigurationValue
{
    ConfigurationValue::Array(vec![
        ConfigurationValue::Object("LowestLabel".to_string(), vec![]),
        ConfigurationValue::Object("EnforceFlowControl".to_string(), vec![]),
        ConfigurationValue::Object("Random".to_string(), vec![])
    ])
}

pub fn create_input_output_router(
    virtual_channels:f64, vcp:ConfigurationValue, delay:f64, allocator: ConfigurationValue, buffer_size: f64, bubble: ConfigurationValue,
    flit_size: f64, allow_request_busy_port: ConfigurationValue, intransit_priority: ConfigurationValue, output_buffer_size: f64, neglect_busy_outport: ConfigurationValue)-> ConfigurationValue
    // router_index: usize, plugs: &Plugs, topology: Box<dyn Topology>, maximum_packet_size: usize, general_frequency_divisor: Time, statistics_temporal_step: Time, rng: &mut StdRng) -> ConfigurationValue
{
    //let plugs = Plugs::default();
    // let router_config =
        ConfigurationValue::Object("InputOutput".to_string(), vec![
            ("virtual_channels".to_string(),ConfigurationValue::Number(virtual_channels)),
            ("virtual_channel_policies".to_string(), vcp),
            ("allocator".to_string(), allocator),
            ("delay".to_string(), ConfigurationValue::Number(delay) ),
            ("buffer_size".to_string(), ConfigurationValue::Number(buffer_size)),
            ("bubble".to_string(), bubble),
            ("flit_size".to_string(), ConfigurationValue::Number(flit_size) ),
            ("allow_request_busy_port".to_string(), allow_request_busy_port),
            ("intransit_priority".to_string(), intransit_priority),
            ("output_buffer_size".to_string(), ConfigurationValue::Number(output_buffer_size)),
            ("neglect_busy_output".to_string(), neglect_busy_outport)
    ])

    // InputOutput::new(router::RouterBuilderArgument{
    //     router_index,
    //     cv: &router_config,
    //     plugs,
    //     topology: topology.as_ref(),
    //     maximum_packet_size,
    //     general_frequency_divisor,
    //     statistics_temporal_step,
    //     rng,
    // })
}

pub fn create_hamming_topology(sides: Vec<ConfigurationValue>, servers_per_router: f64, rng: &mut StdRng) -> ConfigurationValue //Box<dyn Topology>
{
    //let plugs = Plugs::default();
    // let cv =
    ConfigurationValue::Object("Hamming".to_string(),
                                        vec![("sides".to_string(),ConfigurationValue::Array(sides)),
                                             ("servers_per_router".to_string(),ConfigurationValue::Number(servers_per_router))])

    // new_topology(TopologyBuilderArgument{cv:&cv, plugs:&plugs, rng })
}

pub fn create_shift_pattern(sides: Vec<ConfigurationValue>, shift: Vec<ConfigurationValue>) -> ConfigurationValue
{
    ConfigurationValue::Object("CartesianTransform".to_string(), vec![
        ("sides".to_string(),ConfigurationValue::Array(sides)),
        ("shift".to_string(),ConfigurationValue::Array(shift))])
}


pub fn create_burst_traffic(pattern: ConfigurationValue, servers: f64, messages_per_server: f64, message_size: f64) -> ConfigurationValue
{
    ConfigurationValue::Object("Burst".to_string(), vec![("pattern".to_string(),pattern ),
                                                         ("servers".to_string(), ConfigurationValue::Number(servers)),
                                                         ("messages_per_server".to_string(), ConfigurationValue::Number(messages_per_server)),
                                                         ("message_size".to_string(), ConfigurationValue::Number(message_size))])
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

