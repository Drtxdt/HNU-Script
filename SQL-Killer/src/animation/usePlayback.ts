import {ref,watch,onBeforeUnmount,type Ref} from 'vue';
import {PlaybackClock} from './timeline';
export function usePlayback(count:Ref<number>){
 const index=ref(0),progress=ref(0),playing=ref(false),speed=ref(1);let clock=new PlaybackClock(count.value),raf=0;
 function sync(){index.value=clock.index;progress.value=clock.progress;playing.value=clock.playing;}
 function tick(now:number){clock.tick(now);sync();if(clock.playing)raf=requestAnimationFrame(tick);}
 function pause(){clock.pause();cancelAnimationFrame(raf);sync();}
 function play(){cancelAnimationFrame(raf);clock.play();sync();if(clock.playing)raf=requestAnimationFrame(tick);}
 function seek(n:number,end=false){pause();clock.seek(n,end?1:0);sync();}
 function toggle(){if(playing.value)pause();else play();}
 watch(speed,v=>clock.speed=v);
 watch(count,()=>{pause();clock=new PlaybackClock(count.value);clock.speed=speed.value;sync();});
 function visibility(){if(document.hidden)pause();}document.addEventListener('visibilitychange',visibility);
 onBeforeUnmount(()=>{pause();document.removeEventListener('visibilitychange',visibility)});
 return{index,progress,playing,speed,pause,play,seek,toggle};
}
