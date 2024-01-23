
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::prelude::SliceRandom;

//use quantifiable_derive::Quantifiable;//the derive macro
use crate::allocator::{Allocator, Request, GrantedRequests, AllocatorBuilderArgument};
use crate::config_parser::ConfigurationValue;
use crate::match_object_panic;
use crate::pattern::{new_pattern, Pattern, PatternBuilderArgument};
use crate::topology::{new_topology, Topology, TopologyBuilderArgument};


#[derive(Default, Clone)]
struct Resource {
    /// Index of the client that has the resource (or None if the resource is free)
    client: Option<usize>,
}

#[derive(Default, Clone)]
struct Client {
    /// Index of the resource that the client has (or None if the client has no resource)
    resource: Option<usize>,
}
/// A random allocator that randomly allocates requests to resources
pub struct LabelReduction {
    /// The max number of outputs of the router crossbar
    num_resources: usize,
    /// The max number of inputs of the router crossbar
    num_clients: usize,
    /// The requests of the clients
    requests: Vec<Request>,
    ///Labels to be reduced
    labels: Vec<Vec<usize>>,
    /// The patterns to be reduced
    patterns: Vec< Box<dyn Pattern>>,
    ///dummy topology
    topology: Box<dyn Topology>,
    /// The RNG or None if the seed is not set
    rng: Option<StdRng>,
}

impl LabelReduction {
    /// Create a new random priority allocator
    /// # Parameters
    /// * `args` - The arguments for the allocator
    /// # Returns
    /// * `LabelReduction` - The new random priority allocator
    pub fn new(args: AllocatorBuilderArgument) -> LabelReduction {
        // Check if the arguments are valid
        if args.num_clients == 0 || args.num_resources == 0 {
            panic!("Invalid arguments")
        }
        // Get the seed from the configuration
        let mut seed = None;
        let mut labels = vec![];
        let mut patterns = vec![];
        match_object_panic!(args.cv, "LabelReduction", value,
            "seed" => match value
            {
                &ConfigurationValue::Number(s) => seed = Some(s as u64),
                _ => panic!("Bad value for seed"),
            },
            "labels" => match value
            {
                &ConfigurationValue::Array(ref s) => {
                    for i in s {
                        match i {
                            &ConfigurationValue::Array(ref n) => {
                                let mut label = vec![];
                                for j in n {
                                    match j {
                                        &ConfigurationValue::Number(m) => label.push(m as usize),
                                        _ => panic!("Bad value for labels"),
                                    }
                                }
                                labels.push(label);
                            },
                            _ => panic!("Bad value for labels"),
                        }
                    }
                },
                _ => { panic!("Bad value for labels") },
            },
            "patterns" => match value
            {
                &ConfigurationValue::Array(ref s) => {
                    for p in s {
                        patterns.push(new_pattern(PatternBuilderArgument{cv: p, plugs: args.plugs}));
                    }
                },
                _ => panic!("Bad value for patterns"),
            },
        );

        let rng = seed.map(|s| StdRng::seed_from_u64(s));

        if labels.len() != patterns.len() {
            println!("labels: {:?}, patterns: {}", labels, patterns.len());
            panic!("The number of labels and patterns must be the same");
        }
        if labels.len() == 0 {
            panic!("The number of labels and patterns must be greater than 0");
        }

        //dummy hamming
        let cv = ConfigurationValue::Object("Hamming".to_string(), vec![
            ("sides".to_string(),ConfigurationValue::Array(vec![ConfigurationValue::Number(1f64)])),
            ("servers_per_router".to_string(),ConfigurationValue::Number(1f64))
        ]);
        let mut rng_top = StdRng::seed_from_u64(1);
        let topo_builder = TopologyBuilderArgument{cv: &cv, plugs: args.plugs, rng: &mut rng_top };
        let topology = new_topology(topo_builder);



        // Create the allocator
        LabelReduction{
            num_resources: args.num_resources,
            num_clients: args.num_clients,
            requests: Vec::new(),
            labels,
            patterns,
            topology,
            rng,
        }
    }

    /// Check if the request is valid
    /// # Arguments
    /// * `request` - The request to check
    /// # Returns
    /// * `bool` - True if the request is valid, false otherwise
    /// # Remarks
    /// The request is valid if
    /// the client is in the range [0, num_clients) and
    /// the resource is in the range [0, num_resources) and
    /// the priority is is not None
    fn is_valid_request(&self, _request: &Request) -> bool {
        if _request.client >= self.num_clients || _request.resource >= self.num_resources || _request.priority.is_none() {
            return false
        }
        true
    }
}

impl Allocator for LabelReduction {
    /// Add a request to the allocator
    /// # Arguments
    /// * `request` - The request to add
    /// # Remarks
    /// The request is valid if the client is in the range [0, num_clients) and the resource is in the range [0, num_resources) and the priority is is not None
    fn add_request(&mut self, request: Request) {
        // Check if the request is valid
        if !self.is_valid_request(&request) {
            panic!("Invalid request");
        }
        self.requests.push(request);
    }

    /// Perform the allocation
    /// # Arguments
    /// * `rng` - The RNG to use if the seed is not set
    /// # Returns
    /// * `GrantedRequests` - The granted requests
    /// # Remarks
    /// If the seed is not set, the passed RNG is used to generate the random numbers
    /// The granted requests are sorted by priority (from low to high)
    fn perform_allocation(&mut self, rng : &mut StdRng) -> GrantedRequests {
        // Create the granted requests vector
        let mut gr = GrantedRequests::default();
        
        // The resources allocated to the clients
        let mut resources: Vec<Resource> = vec![Resource::default(); self.num_resources];
        // The clients allocated to the resources
        let mut clients: Vec<Client> = vec![Client::default(); self.num_clients];


        // Shuffle the requests using the RNG passed as parameter
        // Except if the seed is set, in which case we use it
        if self.rng.is_none() {
            self.requests.shuffle(rng);
        }else {
            self.requests.shuffle(&mut self.rng.as_mut().unwrap());
        }

        // let rng = self.rng.as_mut().unwrap_or(rng);
        // self.requests.shuffle(rng);
        let mut request_clone = self.requests.clone();
        for (index, p) in self.patterns.iter().enumerate()
        {
            let mut map = self.labels[index].iter().map(|label| (*label, 0usize)).collect::<std::collections::HashMap<_, _>>();
            let pat_ref = p.as_ref();

            request_clone = request_clone.clone().into_iter().filter(|r|{
                let lab = pat_ref.get_destination(r.priority.unwrap(),self.topology.as_ref(), rng).clone();
                if self.labels[index].contains(&lab)
                {
                    if *map.get(&lab).unwrap() == 0usize {
                        map.insert(lab, 1);
                        true
                    }else {
                        false
                    }
                }else {
                    true
                }

            }).collect::<Vec<_>>();
        }
        // let mut map = self.labels.iter().map(|&label| (label, 0usize)).collect::<std::collections::HashMap<_, _>>();

        // let mut cleaned_requests = self.requests.clone().into_iter().filter(|r|{
        //     if self.labels.contains(&r.priority.unwrap())
        //     {
        //         if *map.get(&r.priority.unwrap()).unwrap() == 0usize {
        //            map.insert(r.priority.unwrap(), 1);
        //             true
        //         }else {
        //             false
        //         }
        //     }else {
        //         true
        //     }
        //
        // }).collect::<Vec<_>>();
        let mut cleaned_requests = request_clone;
        // Sort the requests by priority (least is first)
        //self.requests.sort_by(|a, b| a.priority.unwrap().cmp(&b.priority.unwrap()));

        if self.rng.is_none() {
            cleaned_requests.shuffle(rng);
        }else {
            cleaned_requests.shuffle(&mut self.rng.as_mut().unwrap());
        }

        for Request{ref resource, ref client, priority: _ } in cleaned_requests.iter() {
            // Check if the wanted resource is available and if the client has no resource
            if resources[*resource].client.is_none() && clients[*client].resource.is_none() {
                // Add the request to the granted requests
                gr.add_granted_request(Request{
                    client: *client,
                    resource: *resource,
                    priority: None, // Don't care about the priority on this allocator
                });
                // Allocate the resource to the client
                resources[*resource].client = Some(*client);
                // Allocate the client to the resource
                clients[*client].resource = Some(*resource);
            } else {
                // The resource or the client is not available, so we can't grant the request
                continue;
            }
        }
        // Clear the requests vector
        self.requests.clear();
        // Return the granted requests
        gr
    }
    /// Check if the allocator supports the intransit priority option
    fn support_intransit_priority(&self) -> bool {
        true
    }
}

