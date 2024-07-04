use crate::pattern::new_pattern;
use std::cell::{RefCell};
use std::collections::VecDeque;
use std::convert::TryInto;
use ::rand::{Rng,rngs::StdRng};
use std::fs::File;
use std::io::{BufRead,BufReader};
use quantifiable_derive::Quantifiable;//the derive macro
use crate::config_parser::ConfigurationValue;
use crate::topology::cartesian::CartesianData;//for CartesianTransform
use crate::topology::{Topology, Location};
use crate::{match_object_panic};
use crate::pattern::{Pattern, PatternBuilderArgument};


/**
A map read from file. Each node has a unique destination. See [RandomPermutation] for related matters.
The file is read at creation and should contain only lines with pairs `source destination`.

Example configuration:
```ignore
FileMap{
	/// Note this is a string literal.
	filename: "/path/to/pattern",
	legend_name: "A pattern in my device",
}
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct FileMap
{
    permutation: Vec<usize>,
}

impl Pattern for FileMap
{
    fn initialize(&mut self, _source_size:usize, _target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
    {
        //self.permutation=(0..size).collect();
        //rng.shuffle(&mut self.permutation);
    }
    fn get_destination(&self, origin:usize, _topology:&dyn Topology, _rng: &mut StdRng)->usize
    {
        self.permutation[origin]
    }
}

impl FileMap
{
    pub(crate) fn new(arg:PatternBuilderArgument) -> FileMap
    {
        let mut filename=None;
        match_object_panic!(arg.cv,"FileMap",value,
			"filename" => filename = Some(value.as_str().expect("bad value for filename").to_string()),
		);
        let filename=filename.expect("There were no filename");
        let file=File::open(&filename).expect("could not open pattern file.");
        let reader = BufReader::new(&file);
        let mut permutation=Vec::new();
        for rline in reader.lines()
        {
            let line=rline.expect("Some problem when reading the traffic pattern.");
            let mut words=line.split_whitespace();
            let origin=words.next().unwrap().parse::<usize>().unwrap();
            let destination=words.next().unwrap().parse::<usize>().unwrap();
            while permutation.len()<=origin || permutation.len()<=destination
            {
                permutation.push((-1isize) as usize);//which value use as filler?
            }
            permutation[origin]=destination;
        }
        FileMap{
            permutation,
        }
    }
    pub(crate) fn embedded(arg:PatternBuilderArgument) -> FileMap
    {
        let mut map = None;
        match_object_panic!(arg.cv,"EmbeddedMap",value,
			"map" => map = Some(value.as_array()
				.expect("bad value for map").iter()
				.map(|v|v.as_f64().expect("bad value for map") as usize).collect()),
		);
        let permutation = map.expect("There were no map");
        FileMap{
            permutation
        }
    }
}


///Divide the topology according to some given link classes, considering the graph components if the other links were removed.
///Then apply the `global_pattern` among the components and select randomly inside the destination component.
///Note that this uses the topology and will cause problems if used as a sub-pattern.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct ComponentsPattern
{
    component_classes: Vec<usize>,
    //block_pattern: Box<dyn Pattern>,//we would need patterns between places of different extent.
    global_pattern: Box<dyn Pattern>,
    components: Vec<Vec<usize>>,
}

impl Pattern for ComponentsPattern
{
    fn initialize(&mut self, _source_size:usize, _target_size:usize, topology:&dyn Topology, rng: &mut StdRng)
    {
        let mut allowed_components=vec![];
        for link_class in self.component_classes.iter()
        {
            if *link_class>=allowed_components.len()
            {
                allowed_components.resize(*link_class+1,false);
            }
            allowed_components[*link_class]=true;
        }
        self.components=topology.components(&allowed_components);
        //for (i,component) in self.components.iter().enumerate()
        //{
        //	println!("component {}: {:?}",i,component);
        //}
        self.global_pattern.initialize(self.components.len(),self.components.len(),topology,rng);
    }
    fn get_destination(&self, origin:usize, topology:&dyn Topology, rng: &mut StdRng)->usize
    {
        //let local=origin % self.block_size;
        //let global=origin / self.block_size;
        //let n=topology.num_routers();
        let router_origin=match topology.server_neighbour(origin).0
        {
            Location::RouterPort{
                router_index,
                router_port: _,
            } => router_index,
            _ => panic!("what origin?"),
        };
        let mut global=self.components.len();
        for (g,component) in self.components.iter().enumerate()
        {
            if component.contains(&router_origin)
            {
                global=g;
                break;
            }
        }
        if global==self.components.len()
        {
            panic!("Could not found component of {}",router_origin);
        }
        let global_dest=self.global_pattern.get_destination(global,topology,rng);
        //let local_dest=self.block_pattern.get_destination(local,topology,rng);
        let r_local=rng.gen_range(0..self.components[global_dest].len());
        let dest=self.components[global_dest][r_local];
        let radix=topology.ports(dest);
        let mut candidate_stack=Vec::with_capacity(radix);
        for port in 0..radix
        {
            match topology.neighbour(dest,port).0
            {
                Location::ServerPort(destination) => candidate_stack.push(destination),
                _ => (),
            }
        }
        let rserver=rng.gen_range(0..candidate_stack.len());
        candidate_stack[rserver]
    }
}

impl ComponentsPattern
{
    pub(crate) fn new(arg:PatternBuilderArgument) -> ComponentsPattern
    {
        let mut component_classes=None;
        //let mut block_pattern=None;
        let mut global_pattern=None;
        match_object_panic!(arg.cv,"Components",value,
			"global_pattern" => global_pattern=Some(new_pattern(PatternBuilderArgument{cv:value,..arg})),
			"component_classes" => component_classes = Some(value.as_array()
				.expect("bad value for component_classes").iter()
				.map(|v|v.as_f64().expect("bad value in component_classes") as usize).collect()),
		);
        let component_classes=component_classes.expect("There were no component_classes");
        //let block_pattern=block_pattern.expect("There were no block_pattern");
        let global_pattern=global_pattern.expect("There were no global_pattern");
        ComponentsPattern{
            component_classes,
            //block_pattern,
            global_pattern,
            components:vec![],//filled at initialize
        }
    }
}


/**
A pattern that returns in order values recieved from a list of values.
```ignore
InmediateSequencePattern{
    sequence: [0,1,2,3,4,5,6,7,8,9],
}
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct InmediateSequencePattern
{
    sequence: Vec<usize>,
    ///Sequence for each input
    sequences_input: RefCell<Vec<VecDeque<usize>>>,
}

impl Pattern for InmediateSequencePattern
{
    fn initialize(&mut self, source_size:usize, _target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
    {
        self.sequences_input.replace(vec![VecDeque::from(self.sequence.clone()); source_size]);

    }
    fn get_destination(&self, origin:usize, _topology:&dyn Topology, _rng: &mut StdRng)->usize
    {
        self.sequences_input.borrow_mut()[origin].pop_front().unwrap_or(0)
    }
}

impl InmediateSequencePattern
{
    pub(crate) fn new(arg:PatternBuilderArgument) -> InmediateSequencePattern
    {
        let mut sequence=None;
        match_object_panic!(arg.cv,"InmediateSequencePattern",value,
			"sequence" => sequence=Some(value.as_array().expect("bad value for patterns").iter()
				.map(|v|v.as_usize().expect("List should be of usizes")).collect()),
		);
        let sequence = sequence.unwrap();
        InmediateSequencePattern {
            sequence,
            sequences_input: RefCell::new(vec![VecDeque::new()]),
        }
    }
}


/**
For each source, it keeps a state of the last destination used. When applying the pattern, it uses the last destination as the origin for the pattern, and
the destination is saved for the next call to the pattern.
```ignore
ElementComposition{
	pattern: RandomPermutation,
}
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct ElementComposition
{
    ///Pattern to apply.
    pattern: Box<dyn Pattern>,
    ///Pending destinations.
    origin_state: RefCell<Vec<usize>>,
}

impl Pattern for ElementComposition
{
    fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
    {
        if source_size!= target_size
        {
            panic!("ElementComposition requires source and target sets to have same size.");
        }
        self.pattern.initialize(source_size,target_size,_topology,_rng);
        self.origin_state.replace((0..source_size).collect());
    }
    fn get_destination(&self, origin:usize, _topology:&dyn Topology, rng: &mut StdRng)->usize
    {
        if origin >= self.origin_state.borrow().len()
        {
            panic!("ElementComposition: origin {} is beyond the source size {}",origin,self.origin_state.borrow().len());
        }
        let index = self.origin_state.borrow_mut()[origin];
        let destination = self.pattern.get_destination(index,_topology,rng);
        self.origin_state.borrow_mut()[origin] = destination;
        destination
    }
}

impl ElementComposition
{
    pub(crate) fn new(arg:PatternBuilderArgument) -> ElementComposition
    {
        let mut pattern = None;
        match_object_panic!(arg.cv,"ElementComposition",value,
			"pattern" => pattern = Some(new_pattern(PatternBuilderArgument{cv:value,..arg})),
		);
        let pattern = pattern.expect("There were no pattern in configuration of ElementComposition.");
        ElementComposition{
            pattern,
            origin_state: RefCell::new(vec![]),
        }
    }
}
/**
 * Pattern which simulates the communications of an all-gather or all-reduce in log p steps, applying the recursive doubling technique.
 * The communications represent a Hypercube.
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct RecursiveDistanceHalving
{
    ///Pending destinations.
    origin_state: RefCell<Vec<usize>>,
    ///Map for the different states
    cartesian_data: CartesianData,
    ///Order of the neighbours
    neighbours_order: Option<Vec<Vec<usize>>>,
}

impl Pattern for RecursiveDistanceHalving
{
    fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
    {
        if source_size!= target_size
        {
            panic!("RecursiveDistanceHalving requires source and target sets to have same size.");
        }
        //If the source size is not a power of 2, the pattern will not work.
        if !source_size.is_power_of_two()
        {
            panic!("RecursiveDistanceHalving requires source size to be a power of 2.");
        }
        let pow = source_size.ilog2();
        self.origin_state = RefCell::new(vec![0;source_size]);
        self.cartesian_data = CartesianData::new(&(vec![2; pow as usize]))//(0..pow).map(|i| CartesianData::new(&[source_size/2_usize.pow(i), 2_usize.pow(i)]) ).collect();
    }
    fn get_destination(&self, origin:usize, _topology:&dyn Topology, _rng: &mut StdRng)->usize
    {
        if origin >= self.origin_state.borrow().len()
        {
            panic!("RecursiveDistanceHalving: origin {} is beyond the source size {}",origin,self.origin_state.borrow().len());
        }
        let index = self.origin_state.borrow()[origin];
        if index >=self.cartesian_data.sides.len()
        {
            return origin; //No more to do...
        }

        let mut state = self.origin_state.borrow_mut();
        let source_coord = self.cartesian_data.unpack(origin);
        let to_send = if let Some(vectores) = self.neighbours_order.as_ref()
        {
            vectores[state[origin]].clone()
        }else {
            self.cartesian_data.unpack(2_i32.pow(state[origin].try_into().unwrap()) as usize)
        };

        let dest = source_coord.iter().zip(to_send.iter()).map(|(a,b)| a^b).collect::<Vec<usize>>();
        state[origin]+=1;
        self.cartesian_data.pack(&dest)

    }
}

impl RecursiveDistanceHalving
{
    pub(crate) fn new(arg:PatternBuilderArgument) -> RecursiveDistanceHalving
    {
        let mut neighbours_order: Option<Vec<usize>> = None; //Array of vectors which represent the order of the neighbours
        match_object_panic!(arg.cv,"RecursiveDistanceHalving",value,
			"neighbours_order" => neighbours_order = Some(value.as_array().expect("bad value for neighbours_order").iter()
				.map(|n|n.as_usize().unwrap()).collect() ),
		);

        //now each number in the array transform it into an array of binary numbers
        let binary_order = if let Some(n) = neighbours_order
        {
            //get the biggest number
            let max = n.iter().max().unwrap();
            //calculate the number of bits
            let bits = max.ilog2() as usize + 1usize;
            //transform each number into a binary number with the same number of bits
            let bin_n = n.iter().map(|&x| {
                let mut v = vec![0; bits];
                let mut x = x;
                for i in 0..bits
                {
                    v[i] = x%2;
                    x = x/2;
                }
                v
            }).collect();
            Some(bin_n)

        }else{
            None
        };

        RecursiveDistanceHalving{
            origin_state: RefCell::new(vec![]),
            cartesian_data: CartesianData::new(&vec![0;0]),
            neighbours_order: binary_order,
        }
    }
}


/**
 * Pattern to simulate communications in a BinomialTree.
 * Going upwards could be seen as a reduction, and going downwards as a broadcast.
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct BinomialTree
{
    ///How to go through the tree.
    upwards: bool,
    ///Tree embedded into a Hypercube
    cartesian_data: CartesianData,
    ///State indicating the neighbour to send downwards
    state: RefCell<Vec<usize>>,
}

impl Pattern for BinomialTree
{
    fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
    {
        if source_size!= target_size
        {
            panic!("BinomialTree requires source and target sets to have same size.");
        }

        if !source_size.is_power_of_two()
        {
            panic!("BinomialTree requires source size to be a power of 2.");
        }

        let mut tree_order = source_size.ilog2();

        if source_size > 2usize.pow(tree_order)
        {
            tree_order +=1;
        }
        self.cartesian_data = CartesianData::new(&vec![2; tree_order as usize]); // Tree emdebbed into an hypercube
        self.state = RefCell::new(vec![0; source_size]);
    }
    fn get_destination(&self, origin:usize, _topology:&dyn Topology, _rng: &mut StdRng)->usize
    {
        if origin >= self.cartesian_data.size
        {
            panic!("BinomialTree: origin {} is beyond the source size {}",origin,self.cartesian_data.size);
        }
        let mut source_coord = self.cartesian_data.unpack(origin);
        let first_one_index = source_coord.iter().enumerate().find(|(_index, &value)| value == 1);

        return if self.upwards
        {
            if origin == 0 {
                0
            } else {
                let first_one_index = first_one_index.unwrap().0;
                let state = self.state.borrow()[origin];
                if state == 1{
                    origin
                }else{
                    self.state.borrow_mut()[origin] = 1;
                    source_coord[first_one_index] = 0;
                    self.cartesian_data.pack(&source_coord)
                }
            }
        }else{
            let first_one_index = if origin == 0{
                self.cartesian_data.sides.len() //log x in base 2... the number of edges in hypercube
            } else{
                first_one_index.unwrap().0
            };
            let son_index = self.state.borrow()[origin];

            if first_one_index > son_index
            {
                self.state.borrow_mut()[origin] += 1;
                origin + 2usize.pow(son_index as u32)
            }else{
                origin // no sons / no more sons to send
            }
        }
    }
}

impl BinomialTree
{
    pub(crate) fn new(arg:PatternBuilderArgument) -> BinomialTree
    {
        let mut upwards = None;
        match_object_panic!(arg.cv,"BinomialTree",value,
			"upwards" => upwards = Some(value.as_bool().expect("bad value for upwards for pattern BinomialTree")),
		);
        let upwards = upwards.expect("There were no upwards in configuration of BinomialTree.");
        BinomialTree{
            upwards,
            cartesian_data: CartesianData::new(&vec![2;2]),
            state: RefCell::new(vec![]),
        }
    }
}




/**
A transparent meta-pattern to help debug other [Pattern].

```ignore
Debug{
	pattern: ...,
	check_permutation: true,
}
```
 **/
//TODO: admissible, orders/cycle-finding, suprajective,
#[derive(Debug,Quantifiable)]
pub struct DebugPattern {
    /// The pattern being applied transparently.
    pattern: Box<dyn Pattern>,
    /// Whether to consider an error not being a permutation.
    check_permutation: bool,
    /// Size of source cached at initialization.
    source_size: usize,
    /// Size of target cached at initialization.
    target_size: usize,
}

impl Pattern for DebugPattern{
    fn initialize(&mut self, source_size:usize, target_size:usize, topology:&dyn Topology, rng: &mut StdRng)
    {
        self.source_size = source_size;
        self.target_size = target_size;
        self.pattern.initialize(source_size,target_size,topology,rng);
        if self.check_permutation {
            if source_size != target_size {
                panic!("cannot be a permutation is source size {} and target size {} do not agree.",source_size,target_size);
            }
            let mut hits = vec![false;target_size];
            for origin in 0..source_size {
                let dst = self.pattern.get_destination(origin,topology,rng);
                if hits[dst] {
                    panic!("Destination {} hit at least twice.",dst);
                }
                hits[dst] = true;
            }
        }
    }
    fn get_destination(&self, origin:usize, topology:&dyn Topology, rng: &mut StdRng)->usize
    {
        if origin >= self.source_size {
            panic!("Received an origin {origin} beyond source size {size}",size=self.source_size);
        }
        let dst = self.pattern.get_destination(origin,topology,rng);
        if dst >= self.target_size {
            panic!("The destination {dst} is beyond the target size {size}",size=self.target_size);
        }
        dst
    }
}

impl DebugPattern{
    pub(crate) fn new(arg:PatternBuilderArgument) -> DebugPattern{
        let mut pattern = None;
        let mut check_permutation = false;
        match_object_panic!(arg.cv,"Debug",value,
			"pattern" => pattern = Some(new_pattern(PatternBuilderArgument{cv:value,plugs:arg.plugs})),
			"check_permutation" => check_permutation = value.as_bool().expect("bad value for check_permutation"),
		);
        let pattern = pattern.expect("Missing pattern in configuration of Debug.");
        DebugPattern{
            pattern,
            check_permutation,
            source_size:0,
            target_size:0,
        }
    }
}

#[derive(Quantifiable)]
#[derive(Debug)]
pub struct EncapsulatedPattern {}

impl EncapsulatedPattern {
    pub(crate) fn new(pattern: String, arg:PatternBuilderArgument) -> Box<dyn Pattern> {
        let pattern_cv = match pattern.as_str(){
            "Stencil" =>{
                let mut task_space = None;
                match_object_panic!(arg.cv,"Stencil",value,
					"task_space" => task_space = Some(value.as_array().expect("bad value for task_space").iter()
						.map(|v|v.as_usize().expect("bad value in task_space")).collect()),
				);
                let task_space = task_space.expect("There were no task_space in configuration of Stencil.");
                Some(get_stencil_pattern(task_space))
            },
            _ => panic!("Pattern {} not found.",pattern),
        };
        new_pattern(PatternBuilderArgument{cv:&pattern_cv.unwrap(),..arg})
    }
}

pub(crate) fn get_stencil_pattern(task_space: Vec<usize>) -> ConfigurationValue
{
    let space_cv = ConfigurationValue::Array(task_space.iter().map(|&v| ConfigurationValue::Number(v as f64)).collect::<Vec<_>>());

    let mut transforms = vec![];
    for i in 0..task_space.len()
    {
        let mut transform_suc = vec![0;task_space.len()]; //next element in dimension
        transform_suc[i] = 1;
        let transform_suc_cv = ConfigurationValue::Array(transform_suc.iter().map(|&v| ConfigurationValue::Number(v as f64)).collect::<Vec<_>>());

        let mut transform_pred = vec![0;task_space.len()]; //previous element in dimension
        transform_pred[i] = (task_space[i] -1).rem_euclid(task_space[i]);
        let transform_pred_cv = ConfigurationValue::Array(transform_pred.iter().map(|&v| ConfigurationValue::Number(v as f64)).collect::<Vec<_>>());

        transforms.push(
            ConfigurationValue::Object("CartesianTransform".to_string(), vec![
                ("sides".to_string(), space_cv.clone()),
                ("shift".to_string(), transform_suc_cv),
            ]),
        );

        transforms.push(
            ConfigurationValue::Object("CartesianTransform".to_string(), vec![
                ("sides".to_string(), space_cv.clone()),
                ("shift".to_string(), transform_pred_cv),
            ]),
        );
    }

    ConfigurationValue::Object( "RoundRobin".to_string(), vec![
        ("patterns".to_string(), ConfigurationValue::Array(transforms)),
    ])
}


pub fn get_switch_pattern(index_pattern: ConfigurationValue, patterns: Vec<ConfigurationValue>) -> ConfigurationValue{
    ConfigurationValue::Object("Switch".to_string(), vec![
        ("indexing".to_string(), index_pattern),
        ("patterns".to_string(), ConfigurationValue::Array(patterns)),
    ])
}

pub fn get_candidates_selection(pattern: ConfigurationValue, pattern_destination_size: usize) -> ConfigurationValue{
    ConfigurationValue::Object("CandidatesSelection".to_string(), vec![
        ("pattern".to_string(), pattern),
        ("pattern_destination_size".to_string(), ConfigurationValue::Number(pattern_destination_size as f64)),
    ])
}

pub fn get_cartesian_transform(sides: Vec<usize>, shift: Option<Vec<usize>>, patterns: Option<Vec<ConfigurationValue>>) -> ConfigurationValue{
    let mut config = vec![
        ("sides".to_string(), ConfigurationValue::Array(sides.iter().map(|&v| ConfigurationValue::Number(v as f64)).collect::<Vec<_>>())),
    ];
    if let Some(shift) = shift{
        config.push(("shift".to_string(), ConfigurationValue::Array(shift.iter().map(|&v| ConfigurationValue::Number(v as f64)).collect::<Vec<_>>())));
    }
    if let Some(patterns) = patterns{
        config.push(("patterns".to_string(), ConfigurationValue::Array(patterns)));
    }
    ConfigurationValue::Object("CartesianTransform".to_string(), config)
}

pub fn get_hotspot_destination(selected_destinations: Vec<usize>) -> ConfigurationValue{
    ConfigurationValue::Object("Hotspots".to_string(), vec![
        ("destinations".to_string(), ConfigurationValue::Array(selected_destinations.iter().map(|&v| ConfigurationValue::Number(v as f64)).collect::<Vec<_>>()), )
    ])
}


/**
FOR ALEX, NO MASTER
 **/
//TODO: admissible, orders/cycle-finding, suprajective,
#[derive(Debug,Quantifiable)]
pub struct MiDebugPattern {
    /// The pattern being applied transparently.
    pattern: Vec<Box<dyn Pattern>>,
    /// Whether to consider an error not being a permutation.
    check_permutation: bool,
    /// Whether to consider an error not being an injection.
    check_injective: bool,
    /// Size of source cached at initialization.
    source_size: Vec<usize>,
    /// Size of target cached at initialization.
    target_size: usize,
}

impl Pattern for MiDebugPattern {
    fn initialize(&mut self, _source_size:usize, _target_size:usize, topology:&dyn Topology, rng: &mut StdRng)
    {
        // self.source_size = source_size;
        // self.target_size = target_size;
        for (index, pattern) in self.pattern.iter_mut().enumerate() {
            pattern.initialize(self.source_size[index], self.target_size, topology,rng);
        }

        if self.check_injective{

            if self.source_size.iter().sum::<usize>() > self.target_size{
                panic!("cannot be injective if source size {} is more than target size {}",self.source_size.iter().sum::<usize>(),self.target_size);
            }
            let mut hits = vec![-1;self.target_size];
            for (index, size) in self.source_size.iter().enumerate() {

                for origin_local in 0..*size {
                    let dst = self.pattern[index].get_destination(origin_local,topology,rng);
                    if hits[dst] != -1 {
                        panic!("Destination {} hit by origin {}, now by {}, in pattern: {}",dst,hits[dst],origin_local, index);
                    }
                    hits[dst] = origin_local as isize;
                }

            }
            println!("Check injective patterns passed.");
            println!("There were the following number of sources: {:?} ({}), and the following number of destinations: {}",self.source_size,self.source_size.iter().sum::<usize>(),self.target_size);
            println!("There are {} free destinations, and {} servers hits. The free destinations are: {:?}",hits.iter().filter(|x|**x==-1).count(),hits.iter().filter(|x|**x!=-1).count(),hits.iter().enumerate().filter(|(_,x)|**x==-1).map(|(i,_)|i).collect::<Vec<usize>>());

        }
        // if self.check_permutation {
        // 	if self.source_size != self.target_size {
        // 		panic!("cannot be a permutation is source size {} and target size {} do not agree.",self.source_size,self.target_size);
        // 	}
        // 	let mut hits = vec![false;self.target_size];
        // 	for origin in 0..self.source_size {
        // 		let dst = self.pattern.get_destination(origin,topology,rng);
        // 		if hits[dst] {
        // 			panic!("Destination {} hit at least twice.",dst);
        // 		}
        // 		hits[dst] = true;
        // 	}
        // }
        panic!("This is just a check.")
    }
    fn get_destination(&self, _origin:usize, _topology:&dyn Topology, _rng: &mut StdRng)->usize
    {
        0
        // if origin >= self.source_size {
        // 	panic!("Received an origin {origin} beyond source size {size}",size=self.source_size);
        // }
        // let dst = self.pattern.get_destination(origin,topology,rng);
        // if dst >= self.target_size {
        // 	panic!("The destination {dst} is beyond the target size {size}",size=self.target_size);
        // }
        // dst
    }
}

impl MiDebugPattern {
    pub(crate) fn new(arg:PatternBuilderArgument) -> MiDebugPattern {
        let mut pattern = None;
        let mut check_permutation = false;
        let mut check_injective = false;
        let mut source_size = None;
        let mut target_size = None;
        match_object_panic!(arg.cv,"Debug",value,
			"patterns" => pattern = Some(value.as_array().expect("bad value for pattern").iter()
				.map(|pcv|new_pattern(PatternBuilderArgument{cv:pcv,..arg})).collect()),
			"check_permutation" => check_permutation = value.as_bool().expect("bad value for check_permutation"),
			"source_size" => source_size = Some(value.as_array().expect("bad value for source_size").iter()
				.map(|v|v.as_usize().expect("bad value in source_size")).collect()),
			"target_size" => target_size = Some(value.as_usize().expect("bad value for target_size")),
			"check_injective" => check_injective = value.as_bool().expect("bad value for check_injective"),
		);
        let pattern = pattern.expect("Missing pattern in configuration of Debug.");
        let source_size = source_size.expect("Missing source_size in configuration of Debug.");
        let target_size = target_size.expect("Missing target_size in configuration of Debug.");
        MiDebugPattern {
            pattern,
            check_permutation,
            check_injective,
            source_size,
            target_size,
        }
    }
}
