import React, {Component, useContext} from 'react'

import '../scss/settings.scss'
import { ModInfo } from './modinfo'
import AppStateContext from './appcontext'
import App from '../App'
import { debounce } from 'lodash';

interface ISettingsState {
    path: string;
}

export default class Settings extends Component<{}, ISettingsState>
{
    static contextType = AppStateContext
    declare context: React.ContextType<typeof AppStateContext>

    constructor(props:any) {
        super(props);

        this.state = {
            path: "",
        }

        this.debounce_on_path_update = this.debounce_on_path_update.bind(this);
        this.on_update = this.on_update.bind(this);
        this.handle_path_button = this.handle_path_button.bind(this);
    }

    componentDidMount() {
        if (this.state.path === "") {
            this.setState({path : (this.context as App).state.dredgePath!})
        }
    }

    handle_path_button = async() => {
        await this.context?.read_file_contents();
        this.debounce_check_path();
    }

    on_update(value : string) {
        this.setState({path: value});
        this.debounce_on_path_update(value);
    }

    debounce_on_path_update = debounce((value: string) => {
        this.context?.setState({dredgePath: value});
    },
    500)

    debounce_check_path = debounce(() => {
        this.setState({path: (this.context as App).state.dredgePath!})
    },
    500)

    render () {
        return (
            <div className="settings-container">
                <div className="setting">
                    <label>
                        DREDGE Install Location
                    </label>
                    <div className="path">
                        <input 
                            id="setting-path-input"
                            type="text" 
                            className="input-text" 
                            onChange={(e) => {this.on_update(e.target.value)}} 
                            value={this.state.path}
                            />
                        <button className="button" onClick={this.handle_path_button}>...</button>
                    </div>
                </div>
            </div>
        )
    }
}