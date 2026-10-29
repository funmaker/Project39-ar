
macro_rules! mmd_shaders {
	( $( $type:literal $name:ident = $source:literal; )* ) => { $(
		mod $name {
			
			vulkano_shaders::shader! {
				ty: $type,
				path: $source,
				include: [ "src/component/model/mmd/pipeline" ],
				spirv_version: "1.3"
			}
		}
	)* }
}
