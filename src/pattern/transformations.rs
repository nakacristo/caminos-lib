use std::convert::TryInto;
use ::rand::{Rng,rngs::StdRng,prelude::SliceRandom};


use quantifiable_derive::Quantifiable;//the derive macro
use crate::config_parser::ConfigurationValue;
use crate::topology::cartesian::CartesianData;//for CartesianTransform
use crate::topology::{Topology};
use crate::{match_object_panic};
use crate::pattern::{new_pattern, Pattern, PatternBuilderArgument};


///Just set `destination = origin`.
///Mostly to be used inside some meta-patterns.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Identity
{
}

impl Pattern for Identity
{
    fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
    {
        if source_size!=target_size
        {
            unimplemented!("The Identity pattern requires source_size({})=target_size({})",source_size,target_size);
        }
    }
    fn get_destination(&self, origin:usize, _topology:&dyn Topology, _rng: &mut StdRng)->usize
    {
        origin
    }
}

impl Identity
{
    pub(crate) fn new(arg:PatternBuilderArgument) -> Identity
    {
        match_object_panic!(arg.cv,"Identity",_value);
        Identity{
        }
    }
}


/**
Matrix by vector multiplication. Origin is given coordinates as within a block of size `source_size`.
Then the destination coordinate vector is `y=Mx`, with `x` being the origin and `M` the given `matrix`.
This destination vector is converted into an index into a block of size `target_size`.

If the parameter `check_admisible` is true, it will print a warning if the matrix given is not admissible.

Example configuration:
```ignore
LinearTransform{
	source_size: [4,8,8],
	matrix: [
		[1,0,0],
		[0,1,0],
		[0,0,1],
	],
	target_size: [4,8,8],
	legend_name: "Identity",
}
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct LinearTransform
{
    ///The Cartesian interpretation for the source vector
    source_size: CartesianData,
    ///A matrix of integers.
    matrix: Vec<Vec<i32>>,
    ///The Cartesian interpretation for the destination vector
    target_size: CartesianData,
}

impl Pattern for LinearTransform
{
    fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
    {
        if source_size!=self.source_size.size || target_size!=self.target_size.size
        {
            println!("source_size({})!=self.source_size.size({}) || target_size({})!=self.target_size.size({})",source_size,self.source_size.size,target_size,self.target_size.size);
            panic!("Sizes do not agree on LinearTransform.");
        }
        //Check that the number of lines of the matrix is the same as the number of dimensions.
        if self.matrix.len()!=self.target_size.sides.len()
        {
            panic!("The matrix has {} lines, but there are {} dimensions.",self.matrix.len(),self.target_size.sides.len());
        }
        //Check that the size of each line of the matrix is the same as the number of dimensions.
        for (index,line) in self.matrix.iter().enumerate()
        {
            if line.len()!=self.source_size.sides.len()
            {
                panic!("Line {} of the matrix has {} elements, but there are {} dimensions.",index,line.len(),self.source_size.sides.len());
            }
        }
    }
    fn get_destination(&self, origin:usize, _topology:&dyn Topology, _rng: &mut StdRng)->usize
    {
        //use std::convert::TryInto;
        let up_origin=self.source_size.unpack(origin);
        let mut result = vec![0usize;self.target_size.size];
        for (index,value) in self.matrix.iter().enumerate()
        {
            result[index] = (value.iter().zip(up_origin.iter()).map(|(&a, &b)| (a * b as i32)).sum::<i32>().rem_euclid(self.target_size.sides[index] as i32) ) as usize;
        }
        self.target_size.pack(&result)
    }
}

impl LinearTransform
{
    pub(crate) fn new(arg:PatternBuilderArgument) -> LinearTransform
    {
        let mut source_size:Option<Vec<_>>=None;
        let mut matrix:Option<Vec<Vec<i32>>>=None;
        let mut target_size:Option<Vec<_>>=None;
        let mut check_admisible = false;

        match_object_panic!(arg.cv,"LinearTransform",value,
			"source_size" => source_size = Some(value.as_array().expect("bad value for sides").iter()
				.map(|v|v.as_usize().expect("bad value in sides")).collect()),
			"matrix" => matrix=Some(value.as_array().expect("bad value for matrix").iter()
				.map(|v|v.as_array().expect("bad value in matrix").iter().map(|n|n.as_i32().unwrap()).collect() ).collect() ),
			"target_size" => target_size = Some(value.as_array().expect("bad value for sides").iter()
				.map(|v|v.as_usize().expect("bad value in sides")).collect()),
			"check_admisible" => check_admisible = value.as_bool().expect("bad value for check_admisible"),
		);
        let source_size=source_size.expect("There were no sides");
        let matrix=matrix.expect("There were no matrix");
        let target_size=target_size.expect("There were no sides");
        //let permute=permute.expect("There were no permute");
        //let complement=complement.expect("There were no complement");
        //calculate the derminant of the matrix
        if check_admisible{
            let determinant = laplace_determinant(&matrix);
            if determinant == 0
            {
                //print warning
                println!("WARNING: The determinant of the matrix in the LinearTransform is 0.");
            }
        }

        LinearTransform{
            source_size: CartesianData::new(&source_size),
            matrix,
            target_size: CartesianData::new(&target_size),
        }
    }
}


/**
Method to calculate the determinant of a matrix.

TODO: integrate in matrix.rs
 **/
fn laplace_determinant(matrix: &Vec<Vec<i32>>) -> i32
{
    let mut determinant = 0;
    if matrix.len() == 1
    {
        return matrix[0][0];
    }
    else if matrix.len() == 2
    {
        return matrix[0][0] * matrix[1][1] - matrix[0][1] * matrix[1][0];
    }
    else
    {
        for i in 0..matrix.len()
        {
            let mut sub_matrix = vec![vec![0; matrix.len() - 1]; matrix.len() - 1];
            for j in 1..matrix.len()
            {
                let mut index = 0;
                for k in 0..matrix.len()
                {
                    if k == i
                    {
                        continue;
                    }
                    sub_matrix[j - 1][index] = matrix[j][k];
                    index += 1;
                }
            }
            determinant += matrix[0][i] * i32::pow(-1, i as u32) * laplace_determinant(&sub_matrix);
        }
    }
    determinant
}




/**
Build a random permutation on initialization, which is then kept constant.
This allows self-messages; with a reasonable probability of having one.
Has `random_seed` as optional configuration to use an internal random-number generator instead of the simulation-wide one.

See [RandomInvolution] and [FileMap].
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct RandomPermutation
{
    permutation: Vec<usize>,
    rng: Option<StdRng>,
}

impl Pattern for RandomPermutation
{
    fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&dyn Topology, rng: &mut StdRng)
    {
        if source_size!=target_size
        {
            panic!("In a permutation source_size({}) must be equal to target_size({}).",source_size,target_size);
        }
        self.permutation=(0..source_size).collect();
        let rng= self.rng.as_mut().unwrap_or(rng);
        self.permutation.shuffle(rng);
    }
    fn get_destination(&self, origin:usize, _topology:&dyn Topology, _rng: &mut StdRng)->usize
    {
        self.permutation[origin]
    }
}

impl RandomPermutation
{
    pub(crate) fn new(arg:PatternBuilderArgument) -> RandomPermutation
    {
        let mut rng = None;
        match_object_panic!(arg.cv,"RandomPermutation",value,
			"seed" => rng = Some( value.as_rng().expect("bad value for seed") ),
		);
        RandomPermutation{
            permutation: vec![],
            rng,
        }
    }
}

///Build a random involution on initialization, which is then kept constant.
///An involution is a permutation that is a pairing/matching; if `a` is the destination of `b` then `b` is the destination of `a`.
///It will panic if given an odd size.
///See [RandomPermutation].
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct RandomInvolution
{
    permutation: Vec<usize>,
}

impl Pattern for RandomInvolution
{
    fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&dyn Topology, rng: &mut StdRng)
    {
        if source_size!=target_size
        {
            panic!("In a permutation source_size({}) must be equal to target_size({}).",source_size,target_size);
        }
        //self.permutation=(0..source_size).collect();
        //rng.shuffle(&mut self.permutation);
        self.permutation=vec![source_size;source_size];
        //for index in 0..source_size
        //{
        //	if self.permutation[index]==source_size
        //	{
        //		//Look for a partner
        //	}
        //}
        assert_eq!(source_size % 2, 0);
        //Todo: annotate this weird algorithm.
        let iterations=source_size/2;
        let mut max=2;
        for _iteration in 0..iterations
        {
            let first=rng.gen_range(0..max);
            let second=rng.gen_range(0..max-1);
            let (low,high) = if second>=first
            {
                (first,second+1)
            }
            else
            {
                (second,first)
            };
            let mut rep_low = max-2;
            let mut rep_high = max-1;
            if high==rep_low
            {
                rep_high=high;
                rep_low=max-1;
            }
            let mut mate_low=self.permutation[low];
            let mut mate_high=self.permutation[high];
            if mate_low != source_size
            {
                if mate_low==high
                {
                    mate_low=rep_high;
                }
                self.permutation[rep_low]=mate_low;
                self.permutation[mate_low]=rep_low;
            }
            if mate_high != source_size
            {
                if mate_high==low
                {
                    mate_high=rep_low;
                }
                self.permutation[rep_high]=mate_high;
                self.permutation[mate_high]=rep_high;
            }
            self.permutation[low]=high;
            self.permutation[high]=low;
            max+=2;
        }
    }
    fn get_destination(&self, origin:usize, _topology:&dyn Topology, _rng: &mut StdRng)->usize
    {
        self.permutation[origin]
    }
}

impl RandomInvolution
{
    pub(crate) fn new(arg:PatternBuilderArgument) -> RandomInvolution
    {
        match_object_panic!(arg.cv,"RandomInvolution",_value);
        RandomInvolution{
            permutation: vec![],
        }
    }
}

///Build a random map on initialization, which is then kept constant.
///Optionally allow self-messages.
///See `RandomPermutation` and `FileMap`.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct FixedRandom
{
    map: Vec<usize>,
    allow_self: bool,
    rng: Option<StdRng>,
}

impl Pattern for FixedRandom
{
    fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&dyn Topology, rng: &mut StdRng)
    {
        self.map.reserve(source_size);
        let rng= self.rng.as_mut().unwrap_or(rng);
        for source in 0..source_size
        {
            // To avoid selecting self we subtract 1 from the total. If the random falls in the latter half we add it again.
            let n = if self.allow_self || target_size<source { target_size } else { target_size -1 };
            let mut elem = rng.gen_range(0..n);
            if !self.allow_self && elem>=source
            {
                elem += 1;
            }
            self.map.push(elem);
        }
    }
    fn get_destination(&self, origin:usize, _topology:&dyn Topology, _rng: &mut StdRng)->usize
    {
        self.map[origin]
    }
}

impl FixedRandom
{
    pub(crate) fn new(arg:PatternBuilderArgument) -> FixedRandom
    {
        let mut allow_self = false;
        let mut rng = None;
        match_object_panic!(arg.cv,"FixedRandom",value,
			"seed" => rng = Some( value.as_rng().expect("bad value for seed") ),
			"allow_self" => allow_self=value.as_bool().expect("bad value for allow_self"),
		);
        FixedRandom{
            map: vec![],//to be initialized
            allow_self,
            rng,
        }
    }
}

/// Interpretate the origin as with cartesian coordinates. Then add each coordinate with a given factor.
/// It uses default `f64 as usize`, so a small epsilon may be desired.
/// We do not restrict the destination size to be equal to the source size.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct CartesianFactor
{
    ///The Cartesian interpretation.
    cartesian_data: CartesianData,
    ///The coefficient by which it is multiplied each dimension.
    factors: Vec<f64>,
    ///As given in initialization.
    target_size: usize,
}

impl Pattern for CartesianFactor
{
    fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
    {
        self.target_size = target_size;
        if source_size!=self.cartesian_data.size
        {
            panic!("Sizes do not agree on CartesianFactor.");
        }
    }
    fn get_destination(&self, origin:usize, _topology:&dyn Topology, _rng: &mut StdRng)->usize
    {
        let up_origin=self.cartesian_data.unpack(origin);
        let destination = up_origin.iter().zip(self.factors.iter()).map(|(&coord,&f)|coord as f64 * f).sum::<f64>() as usize;
        destination % self.target_size
    }
}

impl CartesianFactor
{
    pub(crate) fn new(arg:PatternBuilderArgument) -> CartesianFactor
    {
        let mut sides: Option<Vec<_>>=None;
        let mut factors=None;
        match_object_panic!(arg.cv,"CartesianFactor",value,
			"sides" => sides=Some(value.as_array().expect("bad value for sides").iter()
				.map(|v|v.as_f64().expect("bad value in sides") as usize).collect()),
			"factors" => factors=Some(value.as_array().expect("bad value for factors").iter()
				.map(|v|v.as_f64().expect("bad value in factors")).collect()),
		);
        let sides=sides.expect("There were no sides");
        let factors=factors.expect("There were no factors");
        CartesianFactor{
            cartesian_data: CartesianData::new(&sides),
            factors,
            target_size:0,
        }
    }
}

/**
Interpretate the origin as with cartesian coordinates and apply transformations.
May permute the dimensions if they have same side.
May complement the dimensions.
Order of composition is: multiplier, shift, permute, complement, project, randomize, pattern. If you need another order you may [compose](Composition) several of them.

Example configuration:
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
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct CartesianTransform
{
    ///The Cartesian interpretation.
    cartesian_data: CartesianData,
    ///A factor multiplying each coordinate. Use 1 for nops.
    multiplier: Option<Vec<i32>>,
    ///A shift to each coordinate, modulo the side. Use 0 for nops.
    shift: Option<Vec<usize>>,
    ///Optionally how dimensions are permuted.
    ///`permute=[0,2,1]` means to permute dimensions 1 and 2, keeping dimension 0 as is.
    permute: Option<Vec<usize>>,
    ///Optionally, which dimensions must be complemented.
    ///`complement=[true,false,false]` means `target_coordinates[0]=side-1-coordinates[0]`.
    complement: Option<Vec<bool>>,
    ///Indicates dimensions to be projected into 0. This causes incast contention.
    project: Option<Vec<bool>>,
    ///Indicates dimensions in which to select a random coordinate.
    ///A random roll performed in each call to `get_destination`.
    random: Option<Vec<bool>>,
    ///Optionally, set a pattern at coordinate. Use Identity for those coordinates with no operation.
    patterns: Option<Vec<Box<dyn Pattern>>>,
}

impl Pattern for CartesianTransform
{
    fn initialize(&mut self, source_size:usize, target_size:usize, topology:&dyn Topology, rng: &mut StdRng)
    {
        if source_size!=target_size
        {
            panic!("In a CartesianTransform source_size({}) must be equal to target_size({}).",source_size,target_size);
        }
        if source_size!=self.cartesian_data.size
        {
            panic!("In a CartesianTransform source_size({}) must be equal to cartesian size({}).",source_size,self.cartesian_data.size);
        }
        if let Some(ref mut patterns) = self.patterns
        {
            for (index,ref mut pat) in patterns.iter_mut().enumerate()
            {
                let coordinate_size = self.cartesian_data.sides[index];
                pat.initialize(coordinate_size, coordinate_size, topology, rng );
            }
        }
    }
    fn get_destination(&self, origin:usize, topology:&dyn Topology, rng: &mut StdRng)->usize
    {
        let up_origin=self.cartesian_data.unpack(origin);
        let up_multiplied=match self.multiplier
        {
            Some(ref v) => v.iter().enumerate().map(|(index,&value)|{
                let dst:i32  = (up_origin[index] as i32*value).rem_euclid(self.cartesian_data.sides[index] as i32);
                dst.try_into().unwrap()
            }).collect(),
            None => up_origin,
        };
        let up_shifted=match self.shift
        {
            Some(ref v) => v.iter().enumerate().map(|(index,&value)|(up_multiplied[index]+value)%self.cartesian_data.sides[index]).collect(),
            None => up_multiplied,
        };
        let up_permuted=match self.permute
        {
            //XXX Should we panic on side mismatch?
            Some(ref v) => v.iter().map(|&index|up_shifted[index]).collect(),
            None => up_shifted,
        };
        let up_complemented=match self.complement
        {
            Some(ref v) => up_permuted.iter().enumerate().map(|(index,&value)|if v[index]{self.cartesian_data.sides[index]-1-value}else {value}).collect(),
            None => up_permuted,
        };
        let up_projected=match self.project
        {
            Some(ref v) => up_complemented.iter().enumerate().map(|(index,&value)|if v[index]{0} else {value}).collect(),
            None => up_complemented,
        };
        let up_randomized=match self.random
        {
            Some(ref v) => up_projected.iter().enumerate().map(|(index,&value)|if v[index]{rng.gen_range(0..self.cartesian_data.sides[index])} else {value}).collect(),
            None => up_projected,
        };
        let up_patterned = match self.patterns
        {
            Some(ref v) => up_randomized.iter().enumerate().map(|(index,&value)|v[index].get_destination(value,topology,rng)).collect(),
            None => up_randomized,
        };
        self.cartesian_data.pack(&up_patterned)
    }
}

impl CartesianTransform
{
    pub(crate) fn new(arg:PatternBuilderArgument) -> CartesianTransform
    {
        let mut sides:Option<Vec<_>>=None;
        let mut shift=None;
        let mut multiplier=None;
        let mut permute=None;
        let mut complement=None;
        let mut project=None;
        let mut random =None;
        let mut patterns=None;
        match_object_panic!(arg.cv,"CartesianTransform",value,
			"sides" => sides = Some(value.as_array().expect("bad value for sides").iter()
				.map(|v|v.as_usize().expect("bad value in sides")).collect()),
			"multiplier" => multiplier=Some(value.as_array().expect("bad value for multiplier").iter()
				.map(|v|v.as_i32().expect("bad value in multiplier") ).collect()),
			"shift" => shift=Some(value.as_array().expect("bad value for shift").iter()
				.map(|v|v.as_usize().expect("bad value in shift") ).collect()),
			"permute" => permute=Some(value.as_array().expect("bad value for permute").iter()
				.map(|v|v.as_usize().expect("bad value in permute") ).collect()),
			"complement" => complement=Some(value.as_array().expect("bad value for complement").iter()
				.map(|v|v.as_bool().expect("bad value in complement")).collect()),
			"project" => project=Some(value.as_array().expect("bad value for project").iter()
				.map(|v|v.as_bool().expect("bad value in project")).collect()),
			"random" => random=Some(value.as_array().expect("bad value for random").iter()
				.map(|v|v.as_bool().expect("bad value in random")).collect()),
			"patterns" => patterns=Some(value.as_array().expect("bad value for patterns").iter()
				.map(|pcv|new_pattern(PatternBuilderArgument{cv:pcv,..arg})).collect()),
		);
        let sides=sides.expect("There were no sides");
        //let permute=permute.expect("There were no permute");
        //let complement=complement.expect("There were no complement");
        CartesianTransform{
            cartesian_data: CartesianData::new(&sides),
            multiplier,
            shift,
            permute,
            complement,
            project,
            random,
            patterns,
        }
    }
}


/// Extend a pattern by giving it a Cartesian representation and a number of repetition periods per dimension.
/// E.g., it may translate a permutation on a 4x4 mesh into a 16x16 mesh.
/// Or it may translate a permutation of routers of a 4x2x2 mesh into a server permutation of 8x8x8x8 by using `[8,2,4,4]` as repetitions.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct CartesianTiling
{
    /// The original pattern.
    pattern: Box<dyn Pattern>,
    /// The Cartesian interpretation of the original pattern.
    base_cartesian_data: CartesianData,
    /// How much to repeat at each dimension.
    repetitions: Vec<usize>,
    /// The final Cartesian representation.
    final_cartesian_data: CartesianData,
}

impl Pattern for CartesianTiling
{
    fn initialize(&mut self, source_size:usize, target_size:usize, topology:&dyn Topology, rng: &mut StdRng)
    {
        let factor: usize = self.repetitions.iter().product();
        assert_eq!(source_size % factor, 0);
        assert_eq!(target_size % factor, 0);
        let base_source_size = source_size / factor;
        let base_target_size = target_size / factor;
        self.pattern.initialize(base_source_size,base_target_size,topology,rng);
    }
    fn get_destination(&self, origin:usize, topology:&dyn Topology, rng: &mut StdRng)->usize
    {
        let up_origin=self.final_cartesian_data.unpack(origin);
        let n=up_origin.len();
        let base_up_origin:Vec<usize> = (0..n).map(|index|up_origin[index]%self.base_cartesian_data.sides[index]).collect();
        let base_origin = self.base_cartesian_data.pack(&base_up_origin);
        let base_destination = self.pattern.get_destination(base_origin,topology,rng);
        let base_up_destination = self.base_cartesian_data.unpack(base_destination);
        let up_destination:Vec<usize> = (0..n).map(|index|{
            let size = self.base_cartesian_data.sides[index];
            let tile = up_origin[index]/size;
            base_up_destination[index] + size*tile
        }).collect();
        self.final_cartesian_data.pack(&up_destination)
    }
}

impl CartesianTiling
{
    pub fn new(arg:PatternBuilderArgument) -> CartesianTiling
    {
        let mut pattern = None;
        let mut sides:Option<Vec<_>>=None;
        let mut repetitions:Option<Vec<_>> = None;
        match_object_panic!(arg.cv,"CartesianTiling",value,
			"pattern" => pattern=Some(new_pattern(PatternBuilderArgument{cv:value,..arg})),
			"sides" => sides = Some(value.as_array().expect("bad value for sides").iter()
				.map(|v|v.as_f64().expect("bad value in sides") as usize).collect()),
			"repetitions" => repetitions = Some(value.as_array().expect("bad value for repetitions").iter()
				.map(|v|v.as_f64().expect("bad value in repetitions") as usize).collect()),
		);
        let pattern=pattern.expect("There were no pattern");
        let sides=sides.expect("There were no sides");
        let repetitions=repetitions.expect("There were no repetitions");
        let n=sides.len();
        assert_eq!(n, repetitions.len());
        let final_sides : Vec<_> = (0..n).map(|index|sides[index]*repetitions[index]).collect();
        CartesianTiling{
            pattern,
            base_cartesian_data: CartesianData::new(&sides),
            repetitions,
            final_cartesian_data: CartesianData::new(&final_sides),
        }
    }
}

/**
Maps from a block into another following the natural embedding, keeping the coordinates of every node.
Both block must have the same number of dimensions, and each dimension should be greater at the destination than at the source.
This is intended to be used to place several small applications in a larger machine.
It can combined with [CartesianTransform] to be placed at an offset, to set a stride, or others.

Example mapping nodes in a block of 16 nodes into one of 64 nodes.
```ignore
CartesianEmbedding{
	source_sides: [4,4],
	destination_sides: [8,8],
}
```
 **/
#[derive(Debug,Quantifiable)]
pub struct CartesianEmbedding
{
    source_cartesian_data: CartesianData,
    destination_cartesian_data: CartesianData,
}

impl Pattern for CartesianEmbedding
{
    fn initialize(&mut self, source_size:usize, target_size:usize, _topology:&dyn Topology, _rng: &mut StdRng)
    {
        if source_size!=self.source_cartesian_data.size
        {
            panic!("Source sizes do not agree on CartesianEmbedding. source_size={source_size}, source_sides={sides:?}",source_size=source_size,sides=self.source_cartesian_data.sides);
        }
        if target_size!=self.destination_cartesian_data.size
        {
            panic!("Destination sizes do not agree on CartesianEmbedding. target_size={target_size}, destinations_sides={sides:?}",target_size=target_size,sides=self.destination_cartesian_data.sides);
        }
    }
    fn get_destination(&self, origin:usize, _topology:&dyn Topology, _rng: &mut StdRng)->usize
    {
        let up_origin=self.source_cartesian_data.unpack(origin);
        self.destination_cartesian_data.pack(&up_origin)
    }
}

impl CartesianEmbedding
{
    pub fn new(arg:PatternBuilderArgument) -> CartesianEmbedding
    {
        let mut source_sides:Option<Vec<_>>=None;
        let mut destination_sides:Option<Vec<_>>=None;
        match_object_panic!(arg.cv,"CartesianEmbedding",value,
			"source_sides" => source_sides = Some(value.as_array().expect("bad value for source_sides").iter()
				.map(|v|v.as_usize().expect("bad value in source_sides")).collect()),
			"destination_sides" => destination_sides = Some(value.as_array().expect("bad value for destination_sides").iter()
				.map(|v|v.as_usize().expect("bad value in destination_sides")).collect()),
		);
        let source_sides=source_sides.expect("There were no source_sides");
        let destination_sides=destination_sides.expect("There were no destination_sides");
        if source_sides.len() != destination_sides.len()
        {
            panic!("Different number of dimensions in CartesianEmbedding.")
        }
        for (index,(ss, ds)) in std::iter::zip( source_sides.iter(), destination_sides.iter() ).enumerate()
        {
            if ss>ds
            {
                panic!("Source is greater than destination at side {index}. {ss}>{ds}",index=index,ss=ss,ds=ds);
            }
        }
        CartesianEmbedding{
            source_cartesian_data: CartesianData::new(&source_sides),
            destination_cartesian_data: CartesianData::new(&destination_sides),
        }
    }
}

/**
Select a block in source/destination sets to send traffic according to a pattern and the remainder according to another. The `uncut_sides` parameter define a large block that may be the whole set, otherwise discarding elements from the end. The `cut_sides` parameter defines a subblock embedded in the former. This defines two sets of nodes: the ones in the subblock and the rest. A pattern can be provided for each of these two sets. It is possible to specify offsets and strides for the subblock.

For example, in a network with 150 servers we could do the following to see it as a `[3,10,5]` block with an `[3,4,3]` block embedded in it. The small block of 36 server selects destinations randomly inside it. The rest of the network, `150-36=114` servers also send randomly among themselves. No message is send between those two sets. The middle dimension has offset 1, so coordinates `[x,0,z]` are out of the small block. It has also stride 2, so it only includes odd `y` coordinates. More precisely, it includes those `[x,y,z]` with any `x`, `z<3`, and `y=2k+1` for `k<4`.
```ignore
CartesianCut{
	uncut_sides: [3,10,5],
	cut_sides: [3,4,3],
	cut_strides: [1,2,1],// defaults to a 1s vector
	cut_offsets: [0,1,0],// defaults to a 0s vector
	cut_pattern: Uniform,
	remainder_pattern: Uniform,//defaults to Identity
}
```
This same example would work for more than 150 servers, putting all that excess in the large set.

Another notable example is to combine several of them. Here, we use a decomposition of the previous whole `[3,10,5]` block into two disjoint blocks of size `[3,5,5]`. The offset is chosen to make sure of both being disjoint (a packing) and covering the whole. Then we select a pattern for each block. Since the two patterns are disjoint the can be [composed](Composition) to obtain a pattern that follows each of the blocks.
```ignore
Composition{patterns:[
	CartesianCut{
		uncut_sides: [3,10,5],
		cut_sides: [3,5,5],
		cut_offsets: [0,0,0],
		cut_pattern: RandomPermutation,
		//remainder_pattern: Identity,
	},
	CartesianCut{
		uncut_sides: [3,10,5],
		cut_sides: [3,5,5],
		cut_offsets: [0,5,0],
		cut_pattern: Uniform,
		//remainder_pattern: Identity,
	},
]}
```
 **/
#[derive(Debug,Quantifiable)]
pub struct CartesianCut
{
    // /// An offset before the block.
    // start_margin: usize,
    // /// Some nodes out of the cube at the end.
    // end_margin: usize,
    /// The source sides. Any node beyond its size goes directly to the remained pattern.
    uncut_cartesian_data: CartesianData,
    /// The block we cut
    cut_cartesian_data: CartesianData,
    /// Offsets to set where the cut start at each dimension. Default to 0.
    cut_offsets: Vec<usize>,
    /// At each dimension cut 1 stripe for each `cut_stride[dim]` uncut cells. Default to 1.
    cut_strides: Vec<usize>,
    /// The pattern over the cut block.
    cut_pattern: Box<dyn Pattern>,
    /// The pattern over the rest.
    remainder_pattern: Box<dyn Pattern>,
}

impl Pattern for CartesianCut
{
    fn initialize(&mut self, source_size:usize, target_size:usize, topology:&dyn Topology, rng: &mut StdRng)
    {
        let cut_size = self.cut_cartesian_data.size;
        self.cut_pattern.initialize(cut_size,cut_size,topology,rng);
        self.remainder_pattern.initialize(source_size-cut_size,target_size-cut_size,topology,rng);
    }
    fn get_destination(&self, origin:usize, topology:&dyn Topology, rng: &mut StdRng)->usize
    {
        let cut_size = self.cut_cartesian_data.size;
        if origin >= self.uncut_cartesian_data.size
        {
            let base = origin - cut_size;
            return self.from_remainder(self.remainder_pattern.get_destination(base,topology,rng));
        }
        let coordinates = self.uncut_cartesian_data.unpack(origin);
        let mut cut_count = 0;
        for dim in (0..coordinates.len()).rev()
        {
            if coordinates[dim] < self.cut_offsets[dim]
            {
                // Coordinate within margin
                return self.from_remainder(self.remainder_pattern.get_destination(origin - cut_count,topology,rng));
            }
            // how many 'rows' of cut are included.
            let hypercut_instances = (coordinates[dim] - self.cut_offsets[dim] + self.cut_strides[dim] -1 ) / self.cut_strides[dim];
            // the size of each 'row'.
            let hypercut_size : usize = self.cut_cartesian_data.sides[0..dim].iter().product();
            if hypercut_instances >= self.cut_cartesian_data.sides[dim]
            {
                // Beyond the cut
                cut_count += self.cut_cartesian_data.sides[dim]*hypercut_size;
                return self.from_remainder(self.remainder_pattern.get_destination(origin - cut_count,topology,rng));
            }
            cut_count += hypercut_instances*hypercut_size;
            if (coordinates[dim] - self.cut_offsets[dim]) % self.cut_strides[dim] != 0
            {
                // Space between stripes
                return self.from_remainder(self.remainder_pattern.get_destination(origin - cut_count,topology,rng));
            }
        }
        self.from_cut(self.cut_pattern.get_destination(cut_count,topology,rng))
    }
}

impl CartesianCut
{
    pub fn new(arg:PatternBuilderArgument) -> CartesianCut
    {
        let mut uncut_sides:Option<Vec<_>>=None;
        let mut cut_sides:Option<Vec<_>>=None;
        let mut cut_offsets:Option<Vec<_>>=None;
        let mut cut_strides:Option<Vec<_>>=None;
        let mut cut_pattern:Option<Box<dyn Pattern>>=None;
        let mut remainder_pattern:Option<Box<dyn Pattern>>=None;
        match_object_panic!(arg.cv,"CartesianCut",value,
			"uncut_sides" => uncut_sides = Some(value.as_array().expect("bad value for uncut_sides").iter()
				.map(|v|v.as_usize().expect("bad value in uncut_sides")).collect()),
			"cut_sides" => cut_sides = Some(value.as_array().expect("bad value for cut_sides").iter()
				.map(|v|v.as_usize().expect("bad value in cut_sides")).collect()),
			"cut_offsets" => cut_offsets = Some(value.as_array().expect("bad value for cut_offsets").iter()
				.map(|v|v.as_usize().expect("bad value in cut_offsets")).collect()),
			"cut_strides" => cut_strides = Some(value.as_array().expect("bad value for cut_strides").iter()
				.map(|v|v.as_usize().expect("bad value in cut_strides")).collect()),
			"cut_pattern" => cut_pattern = Some(new_pattern(arg.with_cv(value))),
			"remainder_pattern" => remainder_pattern = Some(new_pattern(arg.with_cv(value))),
		);
        let uncut_sides=uncut_sides.expect("There were no uncut_sides");
        let cut_sides=cut_sides.expect("There were no cut_sides");
        let n=uncut_sides.len();
        assert_eq!(n,cut_sides.len(),"CartesianCut: dimensions for uncut_sides and cut_sides must match.");
        let cut_offsets = cut_offsets.unwrap_or_else(||vec![0;n]);
        assert_eq!(n,cut_offsets.len(),"CartesianCut: dimensions for cut_offsets do not match.");
        let cut_strides = cut_strides.unwrap_or_else(||vec![1;n]);
        assert_eq!(n,cut_strides.len(),"CartesianCut: dimensions for cut_strides do not match.");
        let cut_pattern = cut_pattern.expect("There were no cut_pattern");
        let remainder_pattern = remainder_pattern.unwrap_or_else(||Box::new(Identity{}));
        CartesianCut{
            uncut_cartesian_data: CartesianData::new(&uncut_sides),
            cut_cartesian_data: CartesianData::new(&cut_sides),
            cut_offsets,
            cut_strides,
            cut_pattern,
            remainder_pattern,
        }
    }
    /**
    From an index in the cut region `(0..self.cut_cartesian_data.size)` get the whole index `0..target_size`.
     **/
    pub fn from_cut(&self, cut_index:usize) -> usize {
        //let hypercut_size : usize = self.cut_cartesian_data.sides[0..dim].iter().product();
        let n = self.cut_cartesian_data.sides.len();
        //let hpercut_sizes : Vec<usize> = Vec::with_capacity(n);
        //hypercut_sizes.push(1);
        //for dim in 1..n {
        //	hypercut_sizes.push( hypercut_sizes[dim-1]*self.cut_cartesian_data.sides[dim-1] );
        //}
        let coordinates = self.cut_cartesian_data.unpack(cut_index);
        let mut whole_index = 0;
        let mut hypersize = 1;
        for dim in 0..n {
            let coordinate = coordinates[dim]*self.cut_strides[dim] + self.cut_offsets[dim];
            whole_index += coordinate*hypersize;
            hypersize *= self.uncut_cartesian_data.sides[dim];
        }
        whole_index
    }
    /**
    From an index in the remainder region `(0..(target_size-self.cut_cartesian_data.size))` get the whole index.
     **/
    pub fn from_remainder(&self, remainder_index:usize) -> usize {
        if remainder_index >= self.uncut_cartesian_data.size {
            return remainder_index;
        }
        //let n = self.cut_cartesian_data.sides.len();
        //let hpercut_sizes : Vec<usize> = Vec::with_capacity(n);
        //hypercut_sizes.push(1);
        //for dim in 1..n {
        //	hypercut_sizes.push( hypercut_sizes[dim-1]*self.cut_cartesian_data.sides[dim-1] );
        //}
        //let mut whole_index = 0;
        ////for dim in (0..n).rev() {
        ////	let remaining_size = hyperuncut_sizes[dim]
        ////	let coordinate = remainder_index - hypercut_sizes[dim];
        ////	whole_index += coordinate*hypersize;
        ////}
        //let mut remaining_size = 0;
        //for dim in 0..n {
        //	// Check if we are in this margin
        //	remaining_size += self.
        //}
        //whole_index
        todo!()
    }
}



/**
Apply some other [Pattern] over a set of nodes whose indices have been remapped according to a [Pattern]-given permutation.
A source `x` chooses as destination `map(pattern(invmap(x)))`, where `map` is the given permutation, `invmap` its inverse and `pattern` is the underlying pattern to apply. In other words, if `pattern(a)=b`, then destination of `map(a)` is set to `map(b)`. It can be seen as a [Composition] that manages building the inverse map.

Remapped nodes requires source and destination to be of the same size. The pattern creating the map is called once and must result in a permutation, as to be able to make its inverse.

For a similar operation on other types see [RemappedServersTopology](crate::topology::operations::RemappedServersTopology).

Example building a cycle in random order.
```ignore
RemappedNodes{
	/// The underlying pattern to be used.
	pattern: Circulant{generators:[1]},
	/// The pattern defining the relabelling.
	map: RandomPermutation,
}
```

 **/
#[derive(Debug,Quantifiable)]
pub struct RemappedNodes
{
    /// Maps from inner indices to outer indices.
    /// It must be a permutation.
    from_base_map: Vec<usize>,
    /// Maps from outer indices to inner indices.
    /// The inverse of `from_base_map`.
    into_base_map: Vec<usize>,
    /// The inner pattern to be applied.
    pattern: Box<dyn Pattern>,
    /// The pattern to build the map vectors.
    map: Box<dyn Pattern>,
}

impl Pattern for RemappedNodes
{
    fn initialize(&mut self, source_size:usize, target_size:usize, topology:&dyn Topology, rng: &mut StdRng)
    {
        if source_size != target_size
        {
            panic!("RemappedNodes requires source and target sets to have same size.");
        }
        let n = source_size;
        self.map.initialize(n,n,topology,rng);
        self.from_base_map = (0..n).map(|inner_index|{
            self.map.get_destination(inner_index,topology,rng)
        }).collect();
        let mut into_base_map = vec![None;n];
        for (inside,&outside) in self.from_base_map.iter().enumerate()
        {
            match into_base_map[outside]
            {
                None => into_base_map[outside]=Some(inside),
                Some(already_inside) => panic!("Two inside nodes ({inside} and {already_inside}) mapped to the same outer index ({outside}).",inside=inside,already_inside=already_inside,outside=outside),
            }
        }
        self.into_base_map = into_base_map.iter().map(|x|x.expect("node not mapped")).collect();
        self.pattern.initialize(n,n,topology,rng);
    }
    fn get_destination(&self, origin:usize, topology:&dyn Topology, rng: &mut StdRng)->usize
    {
        let inner_origin = self.into_base_map[origin];
        let inner_dest = self.pattern.get_destination(inner_origin,topology,rng);
        self.from_base_map[inner_dest]
    }
}

impl RemappedNodes
{
    pub(crate) fn new(arg:PatternBuilderArgument) -> RemappedNodes
    {
        let mut pattern = None;
        let mut map = None;
        match_object_panic!(arg.cv, "RemappedNodes", value,
			"pattern" => pattern = Some(new_pattern(PatternBuilderArgument{cv:value,plugs:arg.plugs})),
			"map" => map = Some(new_pattern(PatternBuilderArgument{cv:value,plugs:arg.plugs})),
		);
        let pattern = pattern.expect("There were no pattern in configuration of RemappedNodes.");
        let map = map.expect("There were no map in configuration of RemappedNodes.");
        RemappedNodes{
            from_base_map: vec![],
            into_base_map: vec![],
            pattern,
            map,
        }
    }
}