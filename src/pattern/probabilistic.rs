use std::cell::{RefCell};
use ::rand::{Rng,rngs::StdRng,prelude::SliceRandom};
use quantifiable_derive::Quantifiable;//the derive macro
use crate::config_parser::ConfigurationValue;
use crate::topology::{Topology, Location};
use crate::{match_object_panic};
use crate::pattern::{new_pattern, Pattern, PatternBuilderArgument};


///Each destination request will be uniform random among all the range `0..size` minus the `origin`.
///Independently of past requests, decisions or origin.
///Has an optional configuration argument `allow_self`, default to false.
///This can be useful for composed patterns, for example, for a group to send uniformly into another group.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct UniformPattern
{
    size: usize,
    allow_self: bool,
}

impl Pattern for UniformPattern
{
    fn initialize(&mut self, _source_size:usize, target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
    {
        self.size=target_size;
    }
    fn get_destination(&self, origin:usize, _topology:&dyn Topology, rng: &mut StdRng)->usize
    {
        let discard_self = !self.allow_self && origin<self.size;
        let random_size = if discard_self { self.size-1 } else { self.size };
        // When discard self, act like self were swapped with the last element.
        // If it were already the last element it is already outside the random range.
        let r=rng.gen_range(0..random_size);
        if discard_self && r==origin {
            random_size
        } else {
            r
        }
    }
}

impl UniformPattern
{
    pub(crate) fn new(arg:PatternBuilderArgument) -> UniformPattern
    {
        let mut allow_self = false;
        match_object_panic!(arg.cv,"Uniform",value,
			"allow_self" => allow_self=value.as_bool().expect("bad value for allow_self"),
		);
        UniformPattern{
            size:0,//to be initialized later
            allow_self,
        }
    }
    pub fn uniform_pattern(allow_target_source: bool) -> UniformPattern
    {
        UniformPattern{
            size:0,//to be initialized later
            allow_self:allow_target_source,
        }
    }
}

/// The destinations are selected from a given pool of servers.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Hotspots
{
    ///The allowed destinations
    destinations: Vec<usize>,
    ///An amount of destinations o be added to the vector on pattern initialization.
    extra_random_destinations: usize
}

impl Pattern for Hotspots
{
    fn initialize(&mut self, _source_size:usize, target_size:usize, _topology:&dyn Topology, rng: &mut StdRng)
    {
        //XXX Do we want to check the user given destinations against target_size?
        for _ in 0..self.extra_random_destinations
        {
            let r=rng.gen_range(0..target_size);
            self.destinations.push(r);
        }
        if self.destinations.is_empty()
        {
            panic!("The Hotspots pattern requires to have at least one destination.");
        }
    }
    fn get_destination(&self, _origin:usize, _topology:&dyn Topology, rng: &mut StdRng)->usize
    {
        let r = rng.gen_range(0..self.destinations.len());
        self.destinations[r]
    }
}

impl Hotspots
{
    pub(crate) fn new(arg:PatternBuilderArgument) -> Hotspots
    {
        let mut destinations=None;
        let mut extra_random_destinations=None;
        match_object_panic!(arg.cv,"Hotspots",value,
			"destinations" => destinations=Some(value.as_array().expect("bad value for destinations").iter()
				.map(|v|v.as_f64().expect("bad value in destinations") as usize).collect()),
			"extra_random_destinations" => extra_random_destinations=Some(
				value.as_f64().unwrap_or_else(|_|panic!("bad value for extra_random_destinations ({:?})",value)) as usize),
		);
        let destinations=destinations.unwrap_or_default();
        let extra_random_destinations=extra_random_destinations.unwrap_or(0);
        Hotspots{
            destinations,
            extra_random_destinations,
        }
    }
}

/// Use either of several patterns, with probability proportional to a weight.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct RandomMix
{
    ///The patterns in the pool to be selected.
    patterns: Vec<Box<dyn Pattern>>,
    ///The given weights, one per pattern.
    weights: Vec<usize>,
    ///A total weight computed at initialization.
    total_weight: usize,
}

impl Pattern for RandomMix
{
    fn initialize(&mut self, source_size:usize, target_size:usize, topology:&dyn Topology, rng: &mut StdRng)
    {
        if self.patterns.len()!=self.weights.len()
        {
            panic!("Number of patterns must match number of weights for the RandomMix meta-pattern.");
        }
        if self.patterns.is_empty()
        {
            panic!("RandomMix requires at least one pattern (and 2 to be sensible).");
        }
        for pat in self.patterns.iter_mut()
        {
            pat.initialize(source_size,target_size,topology,rng);
        }
        self.total_weight=self.weights.iter().sum();
    }
    fn get_destination(&self, origin:usize, topology:&dyn Topology, rng: &mut StdRng)->usize
    {
        let mut w = rng.gen_range(0..self.total_weight);
        let mut index = 0;
        while w>self.weights[index]
        {
            w-=self.weights[index];
            index+=1;
        }
        self.patterns[index].get_destination(origin,topology,rng)
    }
}

impl RandomMix
{
    pub(crate) fn new(arg:PatternBuilderArgument) -> RandomMix
    {
        let mut patterns=None;
        let mut weights=None;
        match_object_panic!(arg.cv,"RandomMix",value,
			"patterns" => patterns=Some(value.as_array().expect("bad value for patterns").iter()
				.map(|pcv|new_pattern(PatternBuilderArgument{cv:pcv,..arg})).collect()),
			"weights" => weights=Some(value.as_array().expect("bad value for weights").iter()
				.map(|v|v.as_f64().expect("bad value in weights") as usize).collect()),
		);
        let patterns=patterns.expect("There were no patterns");
        let weights=weights.expect("There were no weights");
        RandomMix{
            patterns,
            weights,
            total_weight:0,//to be computed later
        }
    }
}


///It keeps a shuffled list, global for all sources, of destinations to which send. Once all have sent it is rebuilt and shuffled again.
///Independently of past requests, decisions or origin.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct GloballyShufflingDestinations
{
    ///Number of destinations.
    size: usize,
    ///Pending destinations.
    pending: RefCell<Vec<usize>>,
}

impl Pattern for GloballyShufflingDestinations
{
    fn initialize(&mut self, _source_size:usize, target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
    {
        self.size=target_size;
        self.pending=RefCell::new(Vec::with_capacity(self.size));
        //if source_size!=target_size
        //{
        //	unimplemented!("Different sizes are not yet implemented for GloballyShufflingDestinations");
        //}
    }
    fn get_destination(&self, _origin:usize, _topology:&dyn Topology, rng: &mut StdRng)->usize
    {
        let mut pending = self.pending.borrow_mut();
        if pending.is_empty()
        {
            for i in 0..self.size
            {
                pending.push(i);
            }
            //rng.shuffle(&mut pending);//rand-0.4
            pending.shuffle(rng);//rand-0.8
        }
        pending.pop().unwrap()
    }
}

impl GloballyShufflingDestinations
{
    pub(crate) fn new(arg:PatternBuilderArgument) -> GloballyShufflingDestinations
    {
        match_object_panic!(arg.cv,"GloballyShufflingDestinations",_value);
        GloballyShufflingDestinations{
            size:0,//to be filled in initialization
            pending:RefCell::new(Vec::new()),//to be filled in initialization
        }
    }
}

///For each group, it keeps a shuffled list of destinations to which send. Once all have sent it is rebuilt and shuffled again.
///Independently of past requests, decisions or origin.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct GroupShufflingDestinations
{
    ///The size of each group.
    group_size: usize,
    ///Number of destinations, in total.
    size: usize,
    ///Pending destinations.
    pending: Vec<RefCell<Vec<usize>>>,
}

impl Pattern for GroupShufflingDestinations
{
    fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
    {
        self.size = target_size;
        let number_of_groups = (source_size+self.group_size-1) / self.group_size;// ts/gs rounded up
        self.pending=vec![RefCell::new(Vec::with_capacity(self.size)) ; number_of_groups];
        //if source_size!=target_size
        //{
        //	unimplemented!("Different sizes are not yet implemented for GroupShufflingDestinations");
        //}
    }
    fn get_destination(&self, origin:usize, _topology:&dyn Topology, rng: &mut StdRng)->usize
    {
        let group = origin / self.group_size;
        let mut pending = self.pending[group].borrow_mut();
        if pending.is_empty()
        {
            for i in 0..self.size
            {
                pending.push(i);
            }
            //rng.shuffle(&mut pending);//rand-0.4
            pending.shuffle(rng);//rand-0.8
        }
        pending.pop().unwrap()
    }
}

impl GroupShufflingDestinations
{
    pub(crate) fn new(arg:PatternBuilderArgument) -> GroupShufflingDestinations
    {
        let mut group_size = None;
        if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
        {
            if cv_name!="GroupShufflingDestinations"
            {
                panic!("A GroupShufflingDestinations must be created from a `GroupShufflingDestinations` object not `{}`",cv_name);
            }
            for &(ref name,ref value) in cv_pairs
            {
                //match name.as_ref()
                match AsRef::<str>::as_ref(&name)
                {
                    "group_size" => match value
                    {
                        &ConfigurationValue::Number(f) => group_size=Some(f as usize),
                        _ => panic!("bad value for group_size"),
                    }
                    "legend_name" => (),
                    _ => panic!("Nothing to do with field {} in GroupShufflingDestinations",name),
                }
            }
        }
        else
        {
            panic!("Trying to create a GroupShufflingDestinations from a non-Object");
        }
        let group_size = group_size.expect("There was no group_size");
        GroupShufflingDestinations{
            group_size,
            size:0,//to be filled in initialization
            pending:vec![],//to be filled in initialization
        }
    }
}

/**
Each message gets its destination sampled uniformly at random among the servers attached to neighbour routers.
It may build a pattern either of servers or switches, controlled through the `switch_level` configuration flag.
This pattern autoscales if requested a size multiple of the network size.

Example configuration:
```ignore
UniformDistance{
	///The distance at which the destination must be from the source.
	distance: 1,
	/// Optionally build the pattern at the switches. This should be irrelevant at direct network with the same number of servers per switch.
	//switch_level: true,
	legend_name: "uniform among neighbours",
}
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct UniformDistance
{
    ///Distance to which destinations must chosen.
    distance: usize,
    ///Whether the pattern is defined at the switches, or otherwise, at the servers.
    switch_level: bool,
    ///sources/destinations mapped to each router/server (depending on `switch_level`).
    concentration: usize,
    ///`pool[i]` contains the routers at `distance` from the router `i`.
    pool: Vec<Vec<usize>>,
}

impl Pattern for UniformDistance
{
    fn initialize(&mut self, source_size:usize, target_size:usize, topology:&dyn Topology, _rng: &mut StdRng)
    {
        let n= if self.switch_level { topology.num_routers() } else { topology.num_servers() };
        //assert!(n==source_size && n==target_size,"The UniformDistance pattern needs source_size({})==target_size({})==num_routers({})",source_size,target_size,n);
        assert_eq!(source_size, target_size, "The UniformDistance pattern needs source_size({})==target_size({})", source_size, target_size);
        assert_eq!(source_size % n, 0, "The UniformDistance pattern needs the number of {}({}) to be a divisor of source_size({})", if self.switch_level { "routers" } else { "servers" }, n, source_size);
        self.concentration = source_size/n;
        self.pool.reserve(n);
        for i in 0..n
        {
            let source = if self.switch_level { i } else {
                match topology.server_neighbour(i).0 {
                    Location::RouterPort{
                        router_index,
                        router_port:_,
                    } => router_index,
                    _ => panic!("unconnected server"),
                }
            };
            let mut found: Vec<usize> = (0..n).filter(|&j|{
                let destination = if self.switch_level { j } else {
                    match topology.server_neighbour(j).0 {
                        Location::RouterPort{
                            router_index,
                            router_port:_,
                        } => router_index,
                        _ => panic!("unconnected server"),
                    }
                };
                topology.distance(source,destination)==self.distance
            }).collect();
            found.shrink_to_fit();
            self.pool.push(found);
        }
    }
    fn get_destination(&self, origin:usize, _topology:&dyn Topology, rng: &mut StdRng)->usize
    {
        let pool = &self.pool[origin/self.concentration];
        let r=rng.gen_range(0..pool.len());
        pool[r]*self.concentration + (origin%self.concentration)
    }
}

impl UniformDistance
{
    pub(crate) fn new(arg:PatternBuilderArgument) -> UniformDistance
    {
        let mut distance =  None;
        let mut switch_level =  false;
        match_object_panic!(arg.cv,"UniformDistance",value,
			"distance" => distance=Some(value.as_f64().expect("bad value for distance") as usize),
			"switch_level" => switch_level = value.as_bool().expect("bad value for switch_level"),
		);
        let distance = distance.expect("There were no distance");
        UniformDistance{
            distance,
            switch_level,
            concentration:0,//to be filled on initialization
            pool: vec![],//to be filled on initialization
        }
    }
}


/**
The node at an `index` sends traffic randomly to one of `index+g`, where `g` is any of the declared `generators`.
These sums are made modulo the destination size, which is intended to be equal the source size.
the induced communication matrix is a Circulant matrix, hence its name.

In this example each node `x` send to either `x+1` or `x+2`.
```ignore
Circulant{
	generators: [1,2],
}
```
 **/
#[derive(Quantifiable,Debug)]
pub struct Circulant
{
    //config:
    ///The generators to be employed.
    pub generators: Vec<i32>,
    //initialized:
    ///The size of the destinations set, captured at initialization.
    pub size: i32,
}

impl Pattern for Circulant
{
    fn initialize(&mut self, _source_size:usize, target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
    {
        self.size = target_size as i32;
    }
    fn get_destination(&self, origin:usize, _topology:&dyn Topology, rng: &mut StdRng)->usize
    {
        let r = rng.gen_range(0..self.generators.len());
        let gen = self.generators[r];
        // Note the '%' operator keeps the argument sign, so we use rem_euclid.
        (origin as i32+gen).rem_euclid(self.size) as usize
    }
}

impl Circulant
{
    pub(crate) fn new(arg:PatternBuilderArgument) -> Circulant
    {
        let mut generators = vec![];
        match_object_panic!(arg.cv,"Circulant",value,
			"generators" => generators=value.as_array().expect("bad value for generators").iter()
				.map(|v|v.as_i32().expect("bad value in generators")).collect(),
		);
        if generators.is_empty()
        {
            panic!("cannot build a Circulant pattern with empty set of generators.");
        }
        Circulant{
            generators,
            size:0,
        }
    }
}

/**
A pattern in which the destinations are randomly sampled from the destinations for which there are some middle router satisfying
some criteria. Note this is only a pattern, the actual packet route does not have to go through such middle router.
It has the same implicit concentration scaling as UniformDistance, allowing building a pattern over a multiple of the number of switches.

Example configuration:
```ignore
RestrictedMiddleUniform{
	/// An optional integer value to allow only middle routers whose index is greater or equal to it.
	minimum_index: 100,
	/// An optional integer value to allow only middle routers whose index is lower or equal to it.
	// maximum_index: 100,
	/// Optionally, give a vector with the possible values of the distance from the source to the middle.
	distances_to_source: [1],
	/// Optionally, give a vector with the possible values of the distance from the middle to the destination.
	distances_to_destination: [1],
	/// Optionally, a vector with distances from source to destination, ignoring middle.
	distances_source_to_destination: [2],
	/// Optionally, set a pattern for those sources with no legal destination.
	else: Uniform,
}
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct RestrictedMiddleUniform
{
    minimum_index: Option<usize>,
    maximum_index: Option<usize>,
    distances_to_source: Option<Vec<usize>>,
    distances_to_destination: Option<Vec<usize>>,
    distances_source_to_destination: Option<Vec<usize>>,
    else_pattern: Option<Box<dyn Pattern>>,
    ///Whether the pattern is defined at the switches, or otherwise, at the servers.
    switch_level: bool,
    /// sources/destinations mapped to each router. An implicit product to ease the normal case.
    concentration: usize,
    ///`pool[i]` contains the routers at `distance` from the router `i`.
    pool: Vec<Vec<usize>>,
}

impl Pattern for RestrictedMiddleUniform
{
    fn initialize(&mut self, source_size:usize, target_size:usize, topology:&dyn Topology, rng: &mut StdRng)
    {
        let n= if self.switch_level { topology.num_routers() } else { topology.num_servers() };
        //assert!(n==source_size && n==target_size,"The RestrictedMiddleUniform pattern needs source_size({})==target_size({})==num_routers({})",source_size,target_size,n);
        assert_eq!(source_size, target_size, "The RestrictedMiddleUniform pattern needs source_size({})==target_size({})", source_size, target_size);
        assert_eq!(source_size % n, 0, "The RestrictedMiddleUniform pattern needs the number of {}({}) to be a divisor of source_size({})", if self.switch_level { "routers" } else { "servers" }, n, source_size);
        self.concentration = source_size/n;
        self.pool.reserve(n);
        let middle_min = self.minimum_index.unwrap_or(0);
        let middle_max = self.maximum_index.unwrap_or_else(||topology.num_routers()-1);
        for source in 0..n
        {
            let source_switch = if self.switch_level { source } else {
                match topology.server_neighbour(source).0 {
                    Location::RouterPort{
                        router_index,
                        router_port:_,
                    } => router_index,
                    _ => panic!("unconnected server"),
                }
            };
            // --- There are two main ways to proceed:
            // --- to run over the n^2 pairs of source/destination, filtering out by middle.
            // --- to run first over possible middle switches and then over destinations. But with this destinations appear for several middles and have to be cleaned up. This way could be more efficient for small distances if employing the neighbour function.
            //let mut found: Vec<usize> = (middle_min..=middle_max).flat_map(|&middle|{
            //	// First check criteria between source and middle
            //	if let Some(ref dists) = self.distances_to_source
            //	{
            //		let d = topology.distance(source,middle);
            //		if !dists.contains(&d) { return vec![]; }
            //	}
            //	// Now look for the destinations satisfying all the criteria.
            //	(0..n).filter(|destination|{
            //		let mut good = true;
            //		if let Some(ref dists) = self.distances_to_destination
            //		{
            //			let d = topology.distance(middle,destination);
            //			if !dists.contains(&d) { good=false; }
            //		}
            //		// we would add other criteria checks here.
            //		good
            //	}).collect()
            //}).collect();
            let mut found: Vec<usize> = (0..n).filter(|&destination|{
                let destination_switch = if self.switch_level { destination } else {
                    match topology.server_neighbour(destination).0 {
                        Location::RouterPort{
                            router_index,
                            router_port:_,
                        } => router_index,
                        _ => panic!("unconnected server"),
                    }
                };
                for middle in middle_min..=middle_max
                {
                    if let Some(ref dists) = self.distances_to_source
                    {
                        let d = topology.distance(source_switch,middle);
                        if !dists.contains(&d) { continue; }
                    }
                    if let Some(ref dists) = self.distances_to_destination
                    {
                        let d = topology.distance(middle,destination_switch);
                        if !dists.contains(&d) { continue; }
                    }
                    if let Some(ref dists) = self.distances_source_to_destination
                    {
                        let d = topology.distance(source_switch,destination_switch);
                        if !dists.contains(&d) { continue; }
                    }
                    return true;
                }
                false
            }).collect();
            if self.else_pattern.is_none(){
                assert!(!found.is_empty(),"RestrictedMiddleUniform: Empty set of destinations for switch {} and there is no else clause set.",source_switch);
            }
            found.shrink_to_fit();
            self.pool.push(found);
        }
        if let Some(ref mut pat) = self.else_pattern
        {
            pat.initialize(source_size,target_size,topology,rng);
        }
    }
    fn get_destination(&self, origin:usize, topology:&dyn Topology, rng: &mut StdRng)->usize
    {
        let pool = &self.pool[origin/self.concentration];
        if pool.is_empty() {
            self.else_pattern.as_ref().expect("else clause should be set").get_destination(origin,topology,rng)
        } else {
            let r=rng.gen_range(0..pool.len());
            pool[r]*self.concentration + (origin%self.concentration)
        }
    }
}


impl RestrictedMiddleUniform
{
    pub(crate) fn new(arg:PatternBuilderArgument) -> RestrictedMiddleUniform
    {
        let mut minimum_index = None;
        let mut maximum_index = None;
        let mut distances_to_source = None;
        let mut distances_to_destination = None;
        let mut distances_source_to_destination = None;
        let mut else_pattern = None;
        let mut switch_level =  false;
        match_object_panic!(arg.cv,"RestrictedMiddleUniform",value,
			"minimum_index" => minimum_index=Some(value.as_f64().expect("bad value for minimum_index") as usize),
			"maximum_index" => maximum_index=Some(value.as_f64().expect("bad value for maximum_index") as usize),
			"distances_to_source" => distances_to_source=Some(
				value.as_array().expect("bad value for distances_to_source").iter().map(
				|x|x.as_f64().expect("bad value for distances_to_source") as usize
			).collect()),
			"distances_to_destination" => distances_to_destination=Some(
				value.as_array().expect("bad value for distances_to_destination").iter().map(
				|x|x.as_f64().expect("bad value for distances_to_destination") as usize
			).collect()),
			"distances_source_to_destination" => distances_source_to_destination=Some(
				value.as_array().expect("bad value for distances_source_to_destination").iter().map(
				|x|x.as_f64().expect("bad value for distances_source_to_destination") as usize
			).collect()),
			"else" => else_pattern=Some(new_pattern(PatternBuilderArgument{cv:value,..arg})),
			"switch_level" => switch_level = value.as_bool().expect("bad value for switch_level"),
		);
        RestrictedMiddleUniform{
            minimum_index,
            maximum_index,
            distances_to_source,
            distances_to_destination,
            distances_source_to_destination,
            else_pattern,
            switch_level,
            concentration:0,//to be filled on initialization
            pool: vec![],//to be filled on initialization
        }
    }
}