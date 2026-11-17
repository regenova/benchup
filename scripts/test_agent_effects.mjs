#!/usr/bin/env node
// Validate agent feedback on a hardware WebGPU adapter; never build.
import {readFile, writeFile, readdir, mkdtemp, rm} from 'node:fs/promises';
import {spawn} from 'node:child_process';
import {createServer} from 'node:http';
import {tmpdir, homedir} from 'node:os';
import {join} from 'node:path';
const cache=join(homedir(),'.cache/ms-playwright');
const version=(await readdir(cache)).filter(n=>/^chromium-\d+$/.test(n)).sort().reverse()[0];
const executable=process.env.CHROME_BIN??join(cache,version,'chrome-linux64/chrome');
const profile=await mkdtemp(join(tmpdir(),'benchup-agent-effects-'));
const server=createServer((_request,response)=>{response.writeHead(200,{'content-type':'text/html'});response.end('<!doctype html><title>Agent GPU regression</title>')});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const browser=spawn(executable,['--headless=new','--no-sandbox','--disable-dev-shm-usage',
 '--enable-unsafe-webgpu','--ignore-gpu-blocklist','--use-angle=vulkan','--enable-features=Vulkan',
 '--remote-debugging-port=0',`--user-data-dir=${profile}`,'about:blank'],{stdio:['ignore','ignore','ignore']});
let socket;
try {
 const deadline=Date.now()+15000;
 let port;
 while(!port&&Date.now()<deadline){try{port=(await readFile(join(profile,'DevToolsActivePort'),'utf8')).split('\n')[0]}catch{await new Promise(r=>setTimeout(r,100))}}
 if(!port)throw Error('Chromium did not start');
 const tabs=await(await fetch(`http://127.0.0.1:${port}/json/list`)).json();
 socket=new WebSocket(tabs.find(t=>t.type==='page').webSocketDebuggerUrl);
 await new Promise((resolve,reject)=>{socket.onopen=resolve;socket.onerror=reject});
 let sequence=0;
 const pending=new Map();
 socket.onmessage=event=>{const m=JSON.parse(event.data);const p=pending.get(m.id);if(p){pending.delete(m.id);m.error?p.reject(Error(JSON.stringify(m.error))):p.resolve(m.result)}};
 const call=(method,params={})=>new Promise((resolve,reject)=>{const id=++sequence;pending.set(id,{resolve,reject});socket.send(JSON.stringify({id,method,params}))});
 await call('Page.navigate',{url:`http://127.0.0.1:${server.address().port}/`});
 await new Promise(resolve=>setTimeout(resolve,200));
 const sources=Object.fromEntries(await Promise.all(['surface_appearance.wgsl','agent_effects.wgsl','edges.wgsl','resident_scene.wgsl','agent_robot.wgsl'].map(async n=>[n,await readFile(new URL('../crates/benchup-app/src/'+n,import.meta.url),'utf8')])));
 const expression=`(${validate.toString()})(${JSON.stringify(sources)})`;
 const result=await call('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true});
 if(result.exceptionDetails)throw Error(result.exceptionDetails.exception?.description??JSON.stringify(result.exceptionDetails));
 const {preview,...report}=result.result.value;
 if(preview)await writeFile('/tmp/benchup-agent-feedback.png',Buffer.from(preview.split(',')[1],'base64'));
 console.log(JSON.stringify({...report,preview:'/tmp/benchup-agent-feedback.png'},null,2));
} finally {
 socket?.close();browser.kill();server.close();
 await new Promise(resolve=>browser.exitCode!==null?resolve():browser.once('exit',resolve));
 await rm(profile,{recursive:true,force:true,maxRetries:5,retryDelay:100});
}

async function validate(sources) {
 if(!navigator.gpu)throw Error('WebGPU is unavailable');
 const adapter=await navigator.gpu.requestAdapter();
 if(!adapter||adapter.info.isFallbackAdapter)throw Error('A hardware GPU is required');
 const device=await adapter.requestDevice();
 const modules={};let pipelineCount=0;
 try {
  device.pushErrorScope('validation');
  for(const name of ['edges.wgsl','resident_scene.wgsl','agent_robot.wgsl']){
   const code=sources['surface_appearance.wgsl']+'\n'+sources['agent_effects.wgsl']+'\n'+sources[name];
   const module=device.createShaderModule({label:name,code});
   const info=await module.getCompilationInfo();
   const errors=info.messages.filter(m=>m.type==='error');
   if(errors.length)throw Error(name+': '+errors.map(m=>`${m.lineNum}:${m.linePos} ${m.message}`).join('\n'));
   modules[name]=module;
  }
  const attributes=(formats,locations)=>{
   const bytes={float32:4,float32x2:8,float32x3:12,float32x4:16,uint32:4,uint32x2:8,uint32x4:16,unorm8x4:4};
   let offset=0;return formats.map((format,i)=>{const value={shaderLocation:locations?.[i]??i,offset,format};offset+=bytes[format];return value});
  };
  const compact={arrayStride:32,attributes:attributes(['float32x3','unorm8x4','float32','unorm8x4','float32','float32'],[0,1,2,3,4,7])};
  const material={arrayStride:20,attributes:attributes(['float32x4','unorm8x4'],[5,6])};
  const face={arrayStride:52,attributes:attributes(['float32x3','float32x3','float32x3','unorm8x4','uint32','uint32x2'])};
  const surface={arrayStride:16,attributes:attributes(['float32x3','unorm8x4'],[6,7])};
  const edge={arrayStride:152,stepMode:'instance',attributes:attributes([...Array(10).fill('float32x3'),'unorm8x4','float32','uint32x4','uint32x2'])};
  const ghost={arrayStride:36,attributes:attributes(['float32x3','float32x3','float32x2','uint32'])};
  const make=async(module,vertex,fragment,buffers,format='rgba8unorm',layout='auto')=>{
   pipelineCount++;
   return await device.createRenderPipelineAsync({layout,vertex:{module,entryPoint:vertex,buffers},fragment:{module,entryPoint:fragment,targets:[{format}]},depthStencil:{format:'depth32float',depthWriteEnabled:false,depthCompare:'always'}});
  };
  const projectedLayout=device.createBindGroupLayout({entries:[
   {binding:0,visibility:GPUShaderStage.FRAGMENT,texture:{sampleType:'float'}},
   {binding:1,visibility:GPUShaderStage.FRAGMENT,sampler:{type:'filtering'}},
   {binding:2,visibility:GPUShaderStage.FRAGMENT,buffer:{type:'uniform',minBindingSize:144}}
  ]});
  const projectedPipelineLayout=device.createPipelineLayout({bindGroupLayouts:[projectedLayout]});
  for(const srgb of [false,true])for(const effects of [false,true]){
   for(const [fragment,materials] of [['fragment_main',false],['fragment_edge_depth',false],['fragment_shadow_faces',false],['fragment_material_faces',true],['fragment_material_shadow_faces',true]]){
    await make(modules['edges.wgsl'],materials?(srgb?'vertex_material_srgb':'vertex_material'):(srgb?'vertex_main_srgb':'vertex_main'),fragment+(effects?'_effect':''),materials?[compact,material]:[compact],srgb?'rgba8unorm-srgb':'rgba8unorm',effects?projectedPipelineLayout:'auto');
   }
  }
  for(const [vertex,fragment,buffers,format] of [
   ['face_vertex','face_color',[face]],['material_vertex','material_color',[face,surface]],
   ['face_vertex','fragment_visibility_effect',[face],'rg32uint'],['shadow_vertex','fragment_visibility',[face],'rg32uint'],
   ['edge_vertex','edge_color',[edge]],['edge_vertex','edge_depth',[edge]],
   ['marker_vertex','marker_color',[{arrayStride:28,stepMode:'instance',attributes:attributes(['float32x3','uint32x4'])}]],
  ])await make(modules['resident_scene.wgsl'],vertex,fragment,buffers,format);
  const robot=await make(modules['agent_robot.wgsl'],'robot_vertex','robot_color',[]);
  const departing=await make(modules['agent_robot.wgsl'],'ghost_vertex','ghost_color',[ghost]);
  const overflowFaces=await make(modules['edges.wgsl'],'vertex_main','fragment_faces_effect',[compact],'rgba8unorm',projectedPipelineLayout);
  // Sample the shared overflow shader across all six boundaries, including
  // points on the boundary and inside the very same crossing face.
  const positions=[[0,0,.128],[-.128,0,.128],[.128,0,.128],[0,-.128,.128],[0,.128,.128],[0,0,0],[0,0,.256],
   [-.2,0,.128],[.2,0,.128],[0,-.2,.128],[0,.2,.128],[0,0,-.02],[0,0,.3],[-.05,0,.128],[.05,0,.128]];
  const overflowProbe=device.createShaderModule({code:sources['surface_appearance.wgsl']+`
   @group(0) @binding(0) var<storage,read_write> samples: array<vec4<f32>>;
   @compute @workgroup_size(1) fn probe(@builtin(global_invocation_id) id:vec3<u32>) {
    let points=array<vec3<f32>,${positions.length}>(${positions.map(p=>`vec3<f32>(${p.join(',')})`).join(',')});
    let low=vec4<f32>(-.128,-.128,0.0,1.0);let high=vec4<f32>(.128,.128,.256,0.0);
    let color=reference_overflow_color(points[id.x],low,high);
    let tinted=reference_overflow_tint(vec4<f32>(.2,.3,.4,.6),points[id.x],low,high,false);
    let disabled=reference_overflow_color(points[id.x],vec4<f32>(low.xyz,0.0),high);
    samples[id.x]=vec4<f32>(color.a,tinted.a,tinted.r,disabled.a);
   }`});
  const overflowPipeline=await device.createComputePipelineAsync({layout:'auto',compute:{module:overflowProbe,entryPoint:'probe'}});
  const overflowOutput=device.createBuffer({size:positions.length*16,usage:GPUBufferUsage.STORAGE|GPUBufferUsage.COPY_SRC});
  const overflowRead=device.createBuffer({size:positions.length*16,usage:GPUBufferUsage.COPY_DST|GPUBufferUsage.MAP_READ});
  const overflowBind=device.createBindGroup({layout:overflowPipeline.getBindGroupLayout(0),entries:[{binding:0,resource:{buffer:overflowOutput}}]});
  const overflowEncoder=device.createCommandEncoder(),overflowPass=overflowEncoder.beginComputePass();
  overflowPass.setPipeline(overflowPipeline);overflowPass.setBindGroup(0,overflowBind);overflowPass.dispatchWorkgroups(positions.length);overflowPass.end();
  overflowEncoder.copyBufferToBuffer(overflowOutput,0,overflowRead,0,positions.length*16);device.queue.submit([overflowEncoder.finish()]);
  await overflowRead.mapAsync(GPUMapMode.READ);const overflowValues=new Float32Array(overflowRead.getMappedRange());
  for(let i=0;i<positions.length;i++){
   const outside=i>=7&&i<=12;
   for(const [column,expected] of [[0,outside ? .35 : 0],[1,.6],[2,outside ? .34 : .2],[3,0]]){
    if(Math.abs(overflowValues[i*4+column]-expected)>1e-5)throw Error(`Reference overflow differs at ${i}:${column}`);
   }
  }
  overflowRead.unmap();overflowOutput.destroy();overflowRead.destroy();
  const W=192,H=192;
  const capture=async(pipeline,bind,vertices=null,count=6)=>{
   const image=device.createTexture({size:[W,H],format:'rgba8unorm',usage:GPUTextureUsage.RENDER_ATTACHMENT|GPUTextureUsage.COPY_SRC});
   const depth=device.createTexture({size:[W,H],format:'depth32float',usage:GPUTextureUsage.RENDER_ATTACHMENT});
   const readback=device.createBuffer({size:W*H*4,usage:GPUBufferUsage.COPY_DST|GPUBufferUsage.MAP_READ});
   const encoder=device.createCommandEncoder(),pass=encoder.beginRenderPass({colorAttachments:[{view:image.createView(),clearValue:[0,0,0,0],loadOp:'clear',storeOp:'store'}],depthStencilAttachment:{view:depth.createView(),depthClearValue:1,depthLoadOp:'clear',depthStoreOp:'discard'}});
   pass.setPipeline(pipeline);pass.setBindGroup(0,bind);if(vertices)pass.setVertexBuffer(0,vertices);pass.draw(count);pass.end();
   encoder.copyTextureToBuffer({texture:image},{buffer:readback,bytesPerRow:W*4},[W,H]);device.queue.submit([encoder.finish()]);
   await readback.mapAsync(GPUMapMode.READ);const bytes=new Uint8Array(readback.getMappedRange()).slice();readback.unmap();
   image.destroy();depth.destroy();readback.destroy();return bytes;
  };
  const buffer=(data,usage)=>{const value=device.createBuffer({size:data.byteLength,usage:usage|GPUBufferUsage.COPY_DST});device.queue.writeBuffer(value,0,data);return value};
  const robotData=new Float32Array(32);for(let i=0;i<4;i++)robotData[i*5]=1;
  robotData.set([W,H,1,0],16);robotData.set([96,96,72,0],20);robotData.set([0.055,0.075,0.10,1],28);
  const robotUniform=buffer(robotData,GPUBufferUsage.UNIFORM);
  const robotBind=device.createBindGroup({layout:robot.getBindGroupLayout(0),entries:[{binding:0,resource:{buffer:robotUniform}}]});
  const samples=[];
  for(const [gear,time] of [[0,.2],[0,1.1],[1,.2],[1,.8]]){
   robotData[23]=gear;robotData[25]=time;device.queue.writeBuffer(robotUniform,0,robotData);
   samples.push(await capture(robot,robotBind));
  }
  const difference=(a,b)=>a.reduce((n,v,i)=>n+Number(Math.abs(v-b[i])>15),0);
  const eyeMotion=difference(samples[0],samples[1]),gearMotion=difference(samples[2],samples[3]);
  if(eyeMotion<100||gearMotion<100)throw Error('Robot eyes/gear did not animate');
  const assembly=await make(modules['edges.wgsl'],'vertex_main','fragment_main_effect',[compact],'rgba8unorm',projectedPipelineLayout);
  const data=new ArrayBuffer(96),dv=new DataView(data);
  for(const [i,p] of [[-1,-1],[3,-1],[-1,3]].entries()){
   const base=i*32;dv.setFloat32(base,p[0],true);dv.setFloat32(base+4,p[1],true);dv.setFloat32(base+8,.5,true);
   [145,160,174,255].forEach((v,j)=>dv.setUint8(base+12+j,v));dv.setFloat32(base+24,-1,true);dv.setFloat32(base+28,10,true);
  }
  const geometry=buffer(data,GPUBufferUsage.VERTEX),effectData=new Float32Array(36);
  effectData.set([10,-1,0,1,W,H,1,0]);
  const effectUniform=buffer(effectData,GPUBufferUsage.UNIFORM);
  const empty=device.createTexture({size:[1,1],format:'rgba8unorm',usage:GPUTextureUsage.TEXTURE_BINDING});
  const effectBind=device.createBindGroup({layout:projectedLayout,entries:[{binding:0,resource:empty.createView()},{binding:1,resource:device.createSampler()},{binding:2,resource:{buffer:effectUniform}}]});
  const coverage=[];
  for(const time of [10,10.475,10.96]){
   effectData[0]=time;device.queue.writeBuffer(effectUniform,0,effectData);
   const image=await capture(assembly,effectBind,geometry,3);samples.push(image);
   coverage.push(image.reduce((n,v,i)=>n+Number(i%4===3&&v>0),0)/(W*H));
  }
  if(coverage[0]>.01||coverage[1]<.35||coverage[1]>.65||coverage[2]<.999)throw Error('Incorrect digital reveal: '+coverage);
  effectData[0]=10;effectData[6]=0;device.queue.writeBuffer(effectUniform,0,effectData);
  const expired=await capture(assembly,effectBind,geometry,3);
  if(expired.some((v,i)=>i%4===3&&v!==255))throw Error('Expired vertices reassembled when the timer wrapped');
  // Scanning uses the same shader in ordinary and retained geometry.
  for(let i=0;i<3;i++)dv.setFloat32(i*32+28,-1,true);device.queue.writeBuffer(geometry,0,data);
  const scanRows=[];
  for(const y of [.85,.15,.85,.15]){
   effectData.set([12,y,0,1]);device.queue.writeBuffer(effectUniform,0,effectData);
   const image=await capture(assembly,effectBind,geometry,3);samples.push(image);
   const rows=Array.from({length:H},(_,row)=>image[(row*W+W/2)*4+2]-image[(row*W+W/2)*4]);
   const peak=rows.indexOf(Math.max(...rows));scanRows.push(peak);
   if(Math.abs(peak-y*H)>3)throw Error('Scan band missed expected row: '+JSON.stringify({y,peak}));
  }
  const ghostData=new ArrayBuffer(108),ghostView=new DataView(ghostData);
  for(const [i,p] of [[-1,-1],[3,-1],[-1,3]].entries()){
   ghostView.setFloat32(i*36,p[0],true);ghostView.setFloat32(i*36+4,p[1],true);ghostView.setFloat32(i*36+8,.5,true);
  }
  const ghostGeometry=buffer(ghostData,GPUBufferUsage.VERTEX);
  const ghostBind=device.createBindGroup({layout:departing.getBindGroupLayout(0),entries:[{binding:0,resource:{buffer:robotUniform}}]});
  const dissolveCoverage=[];
  for(const progress of [0,.5,1]){
   robotData[24]=progress;device.queue.writeBuffer(robotUniform,0,robotData);
   const image=await capture(departing,ghostBind,ghostGeometry,3);samples.push(image);
   dissolveCoverage.push(image.reduce((n,v,i)=>n+Number(i%4===3&&v>0),0)/(W*H));
  }
  if(dissolveCoverage[0]<.99||dissolveCoverage[1]<.35||dissolveCoverage[1]>.65||dissolveCoverage[2]!==0)throw Error('Incorrect digital dissolve: '+dissolveCoverage);
  effectData.fill(0);effectData.set([20,-1,0,1,W,H,0,0]);
  for(let i=0;i<4;i++)effectData[8+i*5]=1;
  effectData.set([-.25,-.25,0,1],28);effectData.set([.25,.25,1,0],32);
  device.queue.writeBuffer(effectUniform,0,effectData);
  const overflowImage=await capture(overflowFaces,effectBind,geometry,3);samples.push(overflowImage);
  const pixel=(image,x,y)=>Array.from(image.slice((y*W+x)*4,(y*W+x)*4+4));
  const expect=(actual,expected,label)=>{if(actual.some((v,i)=>Math.abs(v-expected[i])>2))throw Error(label+': '+actual)};
  expect(pixel(overflowImage,W/2,H/2),[145,160,174,255],'inside portion of crossing face');
  expect(pixel(overflowImage,W/8,H/2),[184,115,120,255],'overflowing portion of crossing face');
  const unaffectedEdges=await capture(assembly,effectBind,geometry,3);
  expect(pixel(unaffectedEdges,W/8,H/2),[145,160,174,255],'edges retain their original color');
  effectData[31]=0;device.queue.writeBuffer(effectUniform,0,effectData);
  const hiddenVolume=await capture(overflowFaces,effectBind,geometry,3);
  expect(pixel(hiddenVolume,W/8,H/2),[145,160,174,255],'hiding the volume removes its tint');
  const sheet=new OffscreenCanvas(W*5,(H+32)*Math.ceil(samples.length/5)),ctx=sheet.getContext('2d');
  ctx.fillStyle='#101820';ctx.fillRect(0,0,sheet.width,sheet.height);ctx.font='14px sans-serif';ctx.fillStyle='#d9f8ff';
  const captions=['Looking / 0.2s','Looking / 1.1s','Preparing / 0.2s','Preparing / 0.8s','Reveal / start','Reveal / half','Reveal / complete','Scan / bottom','Scan / top','Scan / bottom','Scan / top','Dissolve / start','Dissolve / half','Dissolve / complete','Reference overflow'];
  for(const [i,bytes] of samples.entries()){
   const pixels=bytes.slice();for(let j=0;j<pixels.length;j+=4)if(pixels[j+3])for(let k=0;k<3;k++)pixels[j+k]=Math.min(255,Math.round(pixels[j+k]*255/pixels[j+3]));
   const image=new OffscreenCanvas(W,H);image.getContext('2d').putImageData(new ImageData(new Uint8ClampedArray(pixels),W,H),0,0);
   const x=(i%5)*W,y=Math.floor(i/5)*(H+32);ctx.drawImage(image,x,y+24);ctx.fillText(captions[i],x+8,y+18);
  }
  const blob=await sheet.convertToBlob({type:'image/png'});
  const preview=await new Promise(resolve=>{const reader=new FileReader();reader.onload=()=>resolve(reader.result);reader.readAsDataURL(blob)});
  const error=await device.popErrorScope();if(error)throw Error(error.message);
  return {passed:true,shader_modules:4,pipelines:pipelineCount,overflow_samples:positions.length,eye_motion_pixels:eyeMotion,gear_motion_pixels:gearMotion,reveal_coverage:coverage,dissolve_coverage:dissolveCoverage,scan_rows:scanRows,adapter:adapter.info.vendor,preview};
 }finally{device.destroy()}
}
