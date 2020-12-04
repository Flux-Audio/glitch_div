// support functions
use std::collections::VecDeque;

/// 1-pole LP filter
pub fn lp (x: f32, cut: f32, dt: f32, y_p: f32) -> f32 {
    return y_p + (dt/(1.0/cut)) * (x - y_p)
}

/// mean of a deque
pub fn mean (x: VecDeque<f32>) -> f32 {
    let sum = x.iter().sum::<f32>();
    let count = x.len() as f32;
    return if (count > 0.0) {sum / count} else {0.0};
}

/// interlace N buffers
pub fn interlace(buffs: &mut Vec<VecDeque<f32>>, num: usize, res: &mut VecDeque<f32>){
    // pre-allocate deque, assuming each input buffer contains at most a single
    // cycle at 20Hz at 44100 sample rate.
    //let mut f_res: VecDeque<f32> = VecDeque::with_capacity(2205*num);

    //let num = buffs.len();

    let lengths: Vec<usize> = buffs.iter().map(|x| x.len()).collect();

    // getting ratios between lengths of buffers and ceil of ratios
    let mut ratios = vec![0.0; num - 1];
    for i in 1..num{ ratios[i - 1] = lengths[i] as f32 / lengths[0] as f32; }
    let mut c_rts: Vec<u32> = ratios.iter().map(|x| x.ceil() as u32).collect(); 

    // indexes and accumulators
    let mut idx: Vec<usize> = vec![0; num];
    let mut acc = vec![0.0; num - 1];

    // process until any buffer is empty
    'outer: loop{
        // floor of accumulator
        let mut f_acc = vec![0; num - 1];

        // process head buffer
        match buffs[0].pop_front(){
            Some(front) => res.push_back(front),
            None => break 'outer,
        }

        // break if head buffer is out of bounds
        //if idx[0] >= lengths[0]{ break 'outer; }

        // process each other buffer
        for i in 0..num - 1{
            acc[i] += ratios[i];
            f_acc[i] = acc[i].floor() as u32;
            acc[i] -= f_acc[i] as f32;

            // append a number of samples proportional to the ratio of lengths
            // of the buffers
            for j in 0..f_acc[i]{
                match buffs[i + 1].pop_front(){
                    Some(front) => res.push_back(front),
                    None => break 'outer,
                }
                //idx[i + 1] += 1;

                // break if out of bounds
                //if idx[i + 1] >= lengths[i + 1]{ break 'outer; }
            }
        }
    }

    // dump the rest of the buffers at the end of the return buffer
    for i in 0..num{
        'inner: loop{
            match buffs[i].pop_front(){
                Some(front) => res.push_back(front),
                None => break 'inner,
            }
        }
    }
}