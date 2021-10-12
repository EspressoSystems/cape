pragma solidity ^0.8.0;

//contract SetMerkleTree {
//    bytes root;
//
//    bytes32 constant EMPTY_HASH = 0;
//    uint256 constant N = 512;
//
//    function check(bytes32[] calldata proof, bytes32 calldata elem)
//        public
//        returns (bool)
//    {
//        bytes32 running_hash = proof[0]; // or -1?
//
//        bytes32 h = elem_hash(elem);
//        bool[] elem_bit_vec = to_bits(elem_hash); // TODO to_bits
//
//        // the path only goes until a terminal node is reached, so skip
//        // part of the bit-vec
//        uint256 start_bit = elem_bit_vec.length - proof.length;
//
//        for (uint256 i = start_bit; i < elem_bit_vec.length; i++) {
//            bytes32 sib = proof[i - start_bit];
//            bool sib_is_left = elem_bit_vec[i];
//            bytes32 l;
//            bytes32 r;
//
//            if (sib_is_left) {
//                l = sib;
//                r = running_hash;
//            } else {
//                l = running_hash;
//                r = sib;
//            }
//            running_hash = branch_hash(l, r);
//        }
//
//        // if &running_hash == root {
//        //     Ok(match &self.terminal_node {
//        //         SetMerkleTerminalNode::EmptySubtree {} => false,
//        //         SetMerkleTerminalNode::Leaf {
//        //             elem: leaf_elem, ..
//        //         } => (leaf_elem == &elem),
//        //     })
//        // } else {
//        //     Err(running_hash)
//        // }
//
//        return true;
//    }
//
//    function elem_hash(bytes calldata elem) public returns (bytes) {
//        // h(canonical_serialize(nul)) where h is Blake2B personalized with “AAPSet Elem”
//    }
//
//    function leaf_hash(bytes calldata elem) public returns (bytes) {
//        // h(canonical_serialize(nul)) where h is Blake2B personalized with “AAPSet Leaf”
//    }
//
//    function branch_hash(bytes calldata left, bytes calldata right)
//        public
//        returns (bytes)
//    {
//        // h("l"||l||"r"||r) where h is Blake2B personalized with “AAPSet Branch”
//    }
//}
