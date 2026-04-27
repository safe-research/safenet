use alloy::sol;

sol! {
   #[sol(rpc)]
   contract Consensus {
        function getCoordinator() public view returns (address);
   }
}
