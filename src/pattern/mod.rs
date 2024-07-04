/*!

A [Pattern] defines the way elements select their destinations.

see [`new_pattern`](fn.new_pattern.html) for documentation on the configuration syntax of predefined patterns.

 */

pub mod extra;
pub mod operations;
pub mod probabilistic;
pub mod transformations;


use ::rand::{rngs::StdRng};

use crate::config_parser::ConfigurationValue;
use crate::topology::{Topology};
use crate::quantify::Quantifiable;
use crate::{Plugs};
use crate::pattern::extra::{BinomialTree, ComponentsPattern, DebugPattern, ElementComposition, EncapsulatedPattern, FileMap, InmediateSequencePattern, MiDebugPattern, RecursiveDistanceHalving};
use crate::pattern::operations::{CandidatesSelection, Composition, DestinationSets, IndependentRegions, Inverse, Pow, ProductPattern, RoundRobin, SubApp, Sum, Switch};
use crate::pattern::probabilistic::{Circulant, GloballyShufflingDestinations, GroupShufflingDestinations, Hotspots, RandomMix, RestrictedMiddleUniform, UniformDistance, UniformPattern};
use crate::pattern::transformations::{CartesianCut, CartesianEmbedding, CartesianFactor, CartesianTiling, CartesianTransform, FixedRandom, Identity, LinearTransform, RandomInvolution, RandomPermutation, RemappedNodes};

/// Some things most uses of the pattern module will use.
pub mod prelude
{
    pub use super::{Pattern,new_pattern,PatternBuilderArgument};
}

///A `Pattern` describes how a set of entities decides destinations into another set of entities.
///The entities are initially servers, but after some operators it may mean router, rows/columns, or other groupings.
///The source and target set may be or not be the same. Or even be of different size.
///Thus, a `Pattern` is a generalization of the mathematical concept of function.
pub trait Pattern : Quantifiable + std::fmt::Debug
{
    //Indices are either servers or virtual things.
    ///Fix the input and output size, providing the topology and random number generator.
    ///Careful with using topology in sub-patterns. For example, it may be misleading to use the dragonfly topology when
    ///building a pattern among groups or a pattern among the routers of a single group.
    ///Even just a pattern of routers instead of a pattern of servers can lead to mistakes.
    ///Read the documentation of the traffic or meta-pattern using the pattern to know what its their input and output.
    fn initialize(&mut self, source_size:usize, target_size:usize, topology:&dyn Topology, rng: &mut StdRng);
    ///Obtain a destination of a source. This will be called repeatedly as the traffic requires destination for its messages.
    fn get_destination(&self, origin:usize, topology:&dyn Topology, rng: &mut StdRng)->usize;
}

///The argument to a builder function of patterns.
#[derive(Debug)]
pub struct PatternBuilderArgument<'a>
{
    ///A ConfigurationValue::Object defining the pattern.
    pub cv: &'a ConfigurationValue,
    ///The user defined plugs. In case the pattern needs to create elements.
    pub plugs: &'a Plugs,
}

impl<'a> PatternBuilderArgument<'a>
{
    fn with_cv<'b>(&'b self, new_cv:&'b ConfigurationValue) -> PatternBuilderArgument<'b>
    {
        PatternBuilderArgument{
            cv: new_cv,
            plugs: self.plugs,
        }
    }
}


/**Build a new pattern. Patterns are maps between two sets which may depend on the RNG. Generally over the whole set of servers, but sometimes among routers or groups. Check the documentation of the parent Traffic/Permutation for its interpretation.

## Roughly uniform patterns

### Uniform

In the [uniform](UniformPattern) pattern all elements have same probability to send to any other.
```ignore
Uniform{
	legend_name: "uniform",
}
```

### GloballyShufflingDestinations

The [GloballyShufflingDestinations] is an uniform-like pattern that avoids repeating the same destination. It keeps a global vector of destinations. It is shuffled and each created message gets its destination from there. Sometimes you may be selected yourself as destination.

```ignore
GloballyShufflingDestinations{
	legend_name: "globally shuffled destinations",
}
```

### GroupShufflingDestinations

The [GroupShufflingDestinations] pattern is alike [GloballyShufflingDestinations] but keeping one destination vector per each group.

```ignore
GroupShufflingDestinations{
	//E.g., if we select `group_size` to be the number of servers per router we are keeping a destination vector for each router.
	group_size: 5,
	legend_name: "router shuffled destinations",
}
```

### UniformDistance

In [UniformDistance] each message gets its destination sampled uniformly at random among the servers attached to neighbour routers.
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

### RestrictedMiddleUniform
[RestrictedMiddleUniform] is a pattern in which the destinations are randomly sampled from the destinations for which there are some middle router satisfying some criteria. Note this is only a pattern, the actual packet route does not have to go through such middle router.
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

## Permutations and maps.
Each element has a unique destination and a unique element from which it is a destination.

### RandomPermutation
The [RandomPermutation] has same chance to generate any permutation
```ignore
RandomPermutation{
	legend_name: "random server permutation",
}
```

### RandomInvolution
The [RandomInvolution] can only generate involutions. This is, if `p` is the permutation then for any element `x`, `p(p(x))=x`.
```ignore
RandomInvolution{
	legend_name: "random server involution",
}
```

### FixedRandom
In [FixedRandom] each source has an independent unique destination. By the "birthday paradox" we can expect several sources to share a destination, causing incast contention.

### FileMap
With [FileMap] a map is read from a file. Each element has a unique destination.
```ignore
FileMap{
	/// Note this is a string literal.
	filename: "/path/to/pattern",
	legend_name: "A pattern in my device",
}
```

### CartesianTransform
With [CartesianTransform] the nodes are seen as in a n-dimensional orthohedro. Then it applies several transformations. When mapping directly servers it may be useful to use as `sides[0]` the number of servers per router.
```ignore
CartesianTransform{
	sides: [4,8,8],
	multiplier: [1,1,1],//optional
	shift: [0,4,0],//optional
	permute: [0,2,1],//optional
	complement: [false,true,false],//optional
	project: [false,false,false],//optional
	//random: [false,false,true],//optional
	//patterns: [Identity,Identity,Circulant{generators:[1,-1]}]//optional
	legend_name: "Some lineal transformation over a 8x8 mesh with 4 servers per router",
}
```

### Hotspots
[Hotspots] builds a pool of hotspots from a given list of `destinations` plus some amount `extra_random_destinations` computed randomly on initialization.
Destinations are randomly selected from such pool.
This causes incast contention, more explicitly than `FixedRandom`.
```ignore
Hotspots{
	//destinations: [],//default empty
	extra_random_destinations: 5,//default 0
	legend_name: "every server send to one of 5 randomly selected hotspots",
}
```

### Circulant
In [Circulant] each node send traffic to the node `current+g`, where `g` is any of the elements given in the vector `generators`. The operations
being made modulo the destination size. Among the candidates one of them is selected in each call with uniform distribution.

In this example each node `x` send to either `x+1` or `x+2`.
```ignore
Circulant{
	generators: [1,2],
}
```

### CartesianEmbedding

[CartesianEmbedding] builds the natural embedding between two blocks, by keeping the coordinate.

Example mapping nodes in a block of 16 nodes into one of 64 nodes.
```ignore
CartesianEmbedding{
	source_sides: [4,4],
	destination_sides: [8,8],
}
```

## meta patterns

### Product
With [Product](ProductPattern) the elements are divided in blocks. Blocks are mapped to blocks by the `global_pattern`. The `block_pattern` must has input and output size equal to `block_size` and maps the specific elements.
```ignore
Product{
	block_pattern: RandomPermutation,
	global_pattern: RandomPermutation,
	block_size: 10,
	legend_name:"permutation of blocks",
}
```

### Components
[Components](ComponentsPattern) divides the topology along link classes. The 'local' pattern is Uniform.
```ignore
Components{
	global_pattern: RandomPermutation,
	component_classes: [0],
	legend_name: "permutation of the induced group by the 0 link class",
}
```

### Composition
The [Composition] pattern allows to concatenate transformations.
```ignore
Composition{
	patterns: [  FileMap{filename: "/patterns/second"}, FileMap{filename: "/patterns/first"}  ]
	legend_name: "Apply first to origin, and then second to get the destination",
}
```


### Pow
A [Pow] is composition of a `pattern` with itself `exponent` times.
```ignore
Pow{
	pattern: FileMap{filename: "/patterns/mypattern"},
	exponent: "3",
	legend_name: "Apply 3 times my pattern",
}
```


### RandomMix
[RandomMix] probabilistically mixes a list of patterns.
```ignore
RandomMix{
	patterns: [Hotspots{extra_random_destinations:10}, Uniform],
	weights: [5,95],
	legend_name: "0.05 chance of sending to the hotspots",
}
```

### IndependentRegions
With [IndependentRegions] the set of nodes is partitioned in independent regions, each with its own pattern. Source and target sizes must be equal.
```ignore
IndependentRegions{
	// An array with the patterns for each region.
	patterns: [Uniform, Hotspots{destinations:[0]}],
	// An array with the size of each region. They must add up to the total size.
	sizes: [100, 50],
	// Alternatively, use relative_sizes. the pattern will be initialized with sizes proportional to these.
	// You must use exactly one of either `sizes` or `relative_sizes`.
	// relative_sizes: [88, 11],
}
```
### RemappedNodes
[RemappedNodes] allows to apply another pattern using indices that are mapped by another pattern.

Example building a cycle in random order.
```ignore
RemappedNodes{
	/// The underlying pattern to be used.
	pattern: Circulant{generators:[1]},
	/// The pattern defining the relabelling.
	map: RandomPermutation,
}
```

### CartesianCut

With [CartesianCut] you see the nodes as block with an embedded block. Then you define a pattern inside the small block and another outside. See [CartesianCut] for details and examples.
 */
pub fn new_pattern(arg:PatternBuilderArgument) -> Box<dyn Pattern>
{
    if let &ConfigurationValue::Object(ref cv_name, ref _cv_pairs)=arg.cv
    {
        if let Some(builder) = arg.plugs.patterns.get(cv_name)
        {
            return builder(arg);
        }
        match cv_name.as_ref()
        {
            "Identity" => Box::new(Identity::new(arg)),
            "Uniform" => Box::new(UniformPattern::new(arg)),
            "RandomPermutation" => Box::new(RandomPermutation::new(arg)),
            "RandomInvolution" => Box::new(RandomInvolution::new(arg)),
            "FileMap" => Box::new(FileMap::new(arg)),
            "EmbeddedMap" => Box::new(FileMap::embedded(arg)),
            "Product" => Box::new(ProductPattern::new(arg)),
            "Components" => Box::new(ComponentsPattern::new(arg)),
            "CartesianTransform" => Box::new(CartesianTransform::new(arg)),
            "LinearTransform" => Box::new(LinearTransform::new(arg)),
            "CartesianTiling" => Box::new(CartesianTiling::new(arg)),
            "Composition" => Box::new(Composition::new(arg)),
            "Pow" => Box::new(Pow::new(arg)),
            "CartesianFactor" => Box::new(CartesianFactor::new(arg)),
            "Hotspots" => Box::new(Hotspots::new(arg)),
            "RandomMix" => Box::new(RandomMix::new(arg)),
            "ConstantShuffle" =>
            {
                println!("WARNING: the name ConstantShuffle is deprecated, use GloballyShufflingDestinations");
                Box::new(GloballyShufflingDestinations::new(arg))
            }
            "GloballyShufflingDestinations" => Box::new(GloballyShufflingDestinations::new(arg)),
            "GroupShufflingDestinations" => Box::new(GroupShufflingDestinations::new(arg)),
            "UniformDistance" => Box::new(UniformDistance::new(arg)),
            "FixedRandom" => Box::new(FixedRandom::new(arg)),
            "IndependentRegions" => Box::new(IndependentRegions::new(arg)),
            "RestrictedMiddleUniform" => Box::new(RestrictedMiddleUniform::new(arg)),
            "Circulant" => Box::new(Circulant::new(arg)),
            "CartesianEmbedding" => Box::new(CartesianEmbedding::new(arg)),
            "CartesianCut" => Box::new(CartesianCut::new(arg)),
            "RemappedNodes" => Box::new(RemappedNodes::new(arg)),
            "Switch" => Box::new(Switch::new(arg)),
            "Debug" => Box::new(DebugPattern::new(arg)),
            "MiDebugPattern" => Box::new(MiDebugPattern::new(arg)),
            "DestinationSets" => Box::new(DestinationSets::new(arg)),
            "ElementComposition" => Box::new(ElementComposition::new(arg)),
            "CandidatesSelection" => Box::new(CandidatesSelection::new(arg)),
            "Sum" => Box::new(Sum::new(arg)),
            "RoundRobin" => Box::new(RoundRobin::new(arg)),
            "Inverse" => Box::new(Inverse::new(arg)),
            "SubApp" => Box::new(SubApp::new(arg)),
            "RecursiveDistanceHalving" => Box::new(RecursiveDistanceHalving::new(arg)),
            "BinomialTree" => Box::new(BinomialTree::new(arg)),
            "InmediateSequencePattern" => Box::new(InmediateSequencePattern::new(arg)),
            "Stencil" => EncapsulatedPattern::new(cv_name.clone(), arg),
            _ => panic!("Unknown pattern {}",cv_name),
        }
    }
    else
    {
        panic!("Trying to create a Pattern from a non-Object");
    }
}

/// In case you want to build a list of patterns but some of them are optional.
pub fn new_optional_pattern(arg:PatternBuilderArgument) -> Option<Box<dyn Pattern>>
{
    if let &ConfigurationValue::Object(ref cv_name, ref _cv_pairs)=arg.cv
    {
        match cv_name.as_ref()
        {
            "None" => None,
            _ => Some(new_pattern(arg))
        }
    }else {
        panic!("Trying to create a Pattern from a non-Object");
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    #[test]
    fn uniform_test()
    {
        let plugs = Plugs::default();
        let mut rng=StdRng::seed_from_u64(10u64);
        use crate::topology::{new_topology,TopologyBuilderArgument};
        // TODO: topology::dummy?
        let topo_cv = ConfigurationValue::Object("Hamming".to_string(),vec![("sides".to_string(),ConfigurationValue::Array(vec![])), ("servers_per_router".to_string(),ConfigurationValue::Number(1.0))]);
        let dummy_topology = new_topology(TopologyBuilderArgument{cv:&topo_cv,plugs:&plugs,rng:&mut rng});
        for origin_size in [10,20]
        {
            for destination_size in [10,20]
            {
                for allow_self in [true,false]
                {
                    let cv_allow_self = if allow_self { ConfigurationValue::True } else { ConfigurationValue::False };
                    let cv = ConfigurationValue::Object("Uniform".to_string(),vec![("allow_self".to_string(),cv_allow_self)]);
                    let arg = PatternBuilderArgument{ cv:&cv, plugs:&plugs };
                    let mut uniform = UniformPattern::new(arg);
                    uniform.initialize(origin_size,destination_size,&*dummy_topology,&mut rng);
                    let sample_size = (origin_size+destination_size)*10;
                    let origin=5;
                    let mut counts = vec![0;destination_size];
                    for _ in 0..sample_size
                    {
                        let destination = uniform.get_destination(origin,&*dummy_topology,&mut rng);
                        assert!(destination<destination_size, "bad destination from {} into {} (allow_self:{}) got {}",origin_size,destination_size,allow_self,destination);
                        counts[destination]+=1;
                    }
                    assert!( (allow_self && counts[origin]>0) || (!allow_self && counts[origin]==0) , "allow_self failing");
                    for (dest,&count) in counts.iter().enumerate()
                    {
                        assert!( dest==origin || count>0, "missing elements at index {} from {} into {} (allow_self:{})",dest,origin_size,destination_size,allow_self);
                    }
                }
            }
        }
    }
    #[test]
    fn fixed_random_self()
    {
        let plugs = Plugs::default();
        let cv = ConfigurationValue::Object("FixedRandom".to_string(),vec![("allow_self".to_string(),ConfigurationValue::True)]);
        let mut rng=StdRng::seed_from_u64(10u64);
        use crate::topology::{new_topology,TopologyBuilderArgument};
        // TODO: topology::dummy?
        let topo_cv = ConfigurationValue::Object("Hamming".to_string(),vec![("sides".to_string(),ConfigurationValue::Array(vec![])), ("servers_per_router".to_string(),ConfigurationValue::Number(1.0))]);
        let dummy_topology = new_topology(TopologyBuilderArgument{cv:&topo_cv,plugs:&plugs,rng:&mut rng});

        for size in [1000]
        {
            let mut count = 0;
            let sizef = size as f64;
            let sample_size = 100;
            let expected_unique = sizef* ( (sizef-1.0)/sizef ).powf(sizef-1.0) * sample_size as f64;
            let mut unique_count = 0;
            for _ in 0..sample_size
            {
                let arg = PatternBuilderArgument{ cv:&cv, plugs:&plugs };
                let mut with_self = FixedRandom::new(arg);
                with_self.initialize(size,size,&*dummy_topology,&mut rng);
                let mut dests = vec![0;size];
                for origin in 0..size
                {
                    let destination = with_self.get_destination(origin,&*dummy_topology,&mut rng);
                    if destination==origin
                    {
                        count+=1;
                    }
                    dests[destination]+=1;
                }
                unique_count += dests.iter().filter(|&&x|x==1).count();
            }
            assert!( count>=sample_size-1,"too few self messages {}, expecting {}",count,sample_size);
            assert!( count<=sample_size+1,"too many self messages {}, expecting {}",count,sample_size);
            assert!( (unique_count as f64) >= expected_unique*0.99 ,"too few unique destinations {}, expecting {}",unique_count,expected_unique);
            assert!( (unique_count as f64) <= expected_unique*1.01 ,"too many unique destinations {}, expecting {}",unique_count,expected_unique);
        }

        let cv = ConfigurationValue::Object("FixedRandom".to_string(),vec![("allow_self".to_string(),ConfigurationValue::False)]);
        for logsize in 1..10
        {
            let arg = PatternBuilderArgument{ cv:&cv, plugs:&plugs };
            let size = 2usize.pow(logsize);
            let mut without_self = FixedRandom::new(arg);
            without_self.initialize(size,size,&*dummy_topology,&mut rng);
            let count = (0..size).filter( |&origin| origin==without_self.get_destination(origin,&*dummy_topology,&mut rng) ).count();
            assert_eq!(count, 0, "Got {} selfs at size {}.", count, size);
        }
    }
}
