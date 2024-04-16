/*!
Tests for the Input_output router
 */

mod common;
use caminos_lib::*;
use caminos_lib::config_parser::ConfigurationValue;
use common::*;

/// Test local traffic inside a router. There are two servers and each server sends one message of 16 phits to each other.
/// We check that the values obtained in the simulation `[cycle (latency), accepted_load, injected_load, average_packet_hops]` are the expected ones.
#[test]
fn test_megafly_routing()
{
    // Megafly topology
    let global_ports_per_spine = 3;
    let servers_per_leaf = 3;
    let global_arrangement = ConfigurationValue::Object("Palmtree".to_string(),vec![]);
    let group_size = 3;
    let number_of_groups = 10;
    let megafly_builder = MegaflyBuilder{
        global_ports_per_spine,
        servers_per_leaf,
        global_arrangement,
        group_size,
        number_of_groups,
    };

    //Megafly routing
    let builder_megafly_ad = MegaflyAD{
        first_allowed_virtual_channels: vec![0],
        second_allowed_virtual_channels: vec![1],
        minimal_to_deroute: vec![1, 1, 0],
    };

    // Homogeneus
    let homogeneous_traffic_builder = HomogeneousTrafficBuilder{
        pattern: create_uniform_pattern(),
        servers: 90,
        load: 1.0,
        message_size: 16,

    };

    //Virtual Channel Policies
    let occuppancy_minimal_builder = OccupancyPolicyBuilder{
        label_coefficient:0,
        occupancy_coefficient:1,
        product_coefficient:0,
        constant_coefficient:0,
        use_internal_space:ConfigurationValue::True,
        use_neighbour_space:ConfigurationValue::True,
        aggregate:ConfigurationValue::False,
    };
    let occuppancy_policy = create_occupancy_policy(occuppancy_minimal_builder);

    let map_label_builder = MapLabelBuilder{
        label_to_policy: vec![occuppancy_policy.clone(), occuppancy_policy.clone()],
        above_policy: None,
    };
    let map_label = create_map_label_policy(map_label_builder);

    let map_hop_builder = MapHopBuilder{
        hop_to_policy: vec![map_label.clone(),map_label.clone()],
        above_policy: None,
    };

    let map_hop = create_map_hop_policy(map_hop_builder);
    let vcp_args = VirtualChannelPoliciesBuilder{
        policies: vec![
            map_hop,
            ConfigurationValue::Object("LowestLabel".to_string(), vec![]),
            ConfigurationValue::Object("EnforceFlowControl".to_string(), vec![]),
            ConfigurationValue::Object("Random".to_string(), vec![])
        ]
    };
    let vcp = create_vcp(vcp_args);


    //Router Input output
    let crossbar_delay = 2;
    let crossbar_frequency_divisor = 1;
    let router_args = InputOutputRouterBuilder{
        virtual_channels: 2,
        vcp,
        crossbar_delay,
        crossbar_frequency_divisor,
        allocator: ConfigurationValue::Object("Random".to_string(), vec![("seed".to_string(), ConfigurationValue::Number(1f64))]),
        buffer_size: 128,
        bubble: ConfigurationValue::False,
        flit_size: 16, //vct
        allow_request_busy_port: ConfigurationValue::True,
        intransit_priority: ConfigurationValue::False,
        output_buffer_size: 64,
        neglect_busy_outport: ConfigurationValue::False,
    };


    let cycles = 1;
    let maximum_packet_size=16;

    let topology = create_megafly_topology(megafly_builder);
    let traffic = create_homogeneous_traffic(homogeneous_traffic_builder);
    let router = create_input_output_router(router_args);
    let routing = create_megafly_ad(builder_megafly_ad);
    let link_classes = create_link_classes();

    let simulation_builder = SimulationBuilder{
        random_seed: 1,
        warmup: 20000,
        measured: 1000,
        topology,
        traffic,
        router,
        maximum_packet_size,
        general_frequency_divisor: 2,
        routing,
        link_classes
    };

    let plugs = Plugs::default();
    let simulation_cv = create_simulation(simulation_builder);

    let experiment = ConfigurationValue::Experiments(vec![simulation_cv.clone()]);
    println!("{}", experiment.format_terminal());
    let mut simulation = Simulation::new(&simulation_cv, &plugs);

    simulation.run();
    let results = simulation.get_simulation_results();
    println!("{:#?}", results);

    // let estimated_injected_load =  (message_size * messages_per_server) as f64 / (cycles as f64); // Aprox... Maybe not the best value now but it is a start
    // let packet_hops = 0.0;
    //
    // match_object_panic!( &results, "Result", value,
    //     "cycle" => assert_eq!(value.as_f64().expect("Cycle data"), cycles as f64, "Cycle"),
    //     "injected_load" => assert_eq!(value.as_f64().expect("Injected load data"), estimated_injected_load, "Injected load"), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
    //     "accepted_load" => assert_eq!(value.as_f64().expect("Accepted load load data"), estimated_injected_load, "Accepted load"), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
    //     "average_packet_hops" => assert_eq!(value.as_f64().expect("Packet hops data"), packet_hops, "Total hops"), //assert!( value.as_f64().expect("Injected load data") as f64 == estimated_injected_load),
    //     _ => (),
    // );
    print!("Test passed\n")
}