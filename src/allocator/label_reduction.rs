
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::prelude::SliceRandom;

//use quantifiable_derive::Quantifiable;//the derive macro
use crate::allocator::{Allocator, Request, GrantedRequests, AllocatorBuilderArgument};
use crate::config_parser::ConfigurationValue;
use crate::config_parser::Token::True;
use crate::match_object_panic;


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
    labels: Vec<usize>,
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
                        &ConfigurationValue::Number(n) => labels.push(n as usize),
                        _ => panic!("Bad value for labels"),
                    }
                }
            },
            _ => panic!("Bad value for labels"),
        },
        );
        let rng = seed.map(|s| StdRng::seed_from_u64(s));
        // Create the allocator
        LabelReduction {
            num_resources: args.num_resources,
            num_clients: args.num_clients,
            requests: Vec::new(),
            labels,
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

        let mut map = self.labels.iter().map(|&label| (label, 0usize)).collect::<std::collections::HashMap<_, _>>();

        let mut cleaned_requests = self.requests.clone().into_iter().filter(|r|{
            if self.labels.contains(&r.priority.unwrap())
            {
                if *map.get(&r.priority.unwrap()).unwrap() == 0usize {
                   map.insert(r.priority.unwrap(), 1);
                    true
                }else {
                    false
                }
            }else {
                true
            }

        }).collect::<Vec<_>>();
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

